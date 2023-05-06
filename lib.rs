#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang::contract;

#[contract]
mod chest {
    use ink_prelude::vec::Vec;
    use ink_storage::{
        collections::HashMap as StorageHashMap,
        traits::{PackedLayout, SpreadLayout},
    };
    use ink_env::{
        self,
        hash::Blake2x256,
        Clear,
        call::{FromAccountId, Selector},
        AccountId,
    };
    use ink_primitives::{
        self,
        U256,
        crypto::hash::{
            Sha2x256,
            HashOutput,
        },
    };

    #[ink(storage)]
    pub struct Chest {
        total_supply: u128,
        name: String,
        symbol: String,
        decimals: u8,
        balances: StorageHashMap<AccountId, u128>,
        allowed: StorageHashMap<(AccountId, AccountId), u128>,
        collateral_pool: u128,
        collateral_address: AccountId,
        collateral_price: u128,
    }

    impl Chest {
        #[ink(constructor)]
        pub fn new(name: String, symbol: String, decimals: u8, collateral_address: AccountId, collateral_price: u128) -> Self {
            let mut instance = Self {
                name,
                symbol,
                decimals,
                balances: StorageHashMap::new(),
                allowed: StorageHashMap::new(),
                total_supply: 0,
                collateral_pool: 0,
                collateral_address,
                collateral_price,
            };
            instance
        }

        #[ink(message)]
        pub fn name(&self) -> String {
            self.name.clone()
        }

        #[ink(message)]
        pub fn symbol(&self) -> String {
            self.symbol.clone()
        }

        #[ink(message)]
        pub fn decimals(&self) -> u8 {
            self.decimals
        }

        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            *self.balances.get(&owner).unwrap_or(&0)
        }

        #[ink(message)]
        pub fn allowance(&self, owner: AccountId, spender: AccountId) -> u128 {
            *self.allowed.get(&(owner, spender)).unwrap_or(&0)
        }

        #[ink(message)]
        pub fn approve(&mut self, spender: AccountId, amount: u128) -> bool {
            let sender = self.env().caller();
            self.allowed.insert((sender, spender), amount);
            self.env().emit_event(Approval {
                owner: sender,
                spender,
                amount,
            });
            true
        }

        #[ink(message)]
        pub fn transfer(&mut self, to: AccountId, amount: u128) -> bool {
            let sender = self.env().caller();
            self.transfer_from_to(sender, to, amount)
        }

        #[ink(message)]
        pub fn transfer_from(&mut self, from: AccountId, to: AccountId, amount: u128) -> bool {
            let sender = self.env().caller();
            let allowance = self.allowed.get(&(from, sender)).cloned().unwrap_or(0);
            assert!(allowance >= amount, "Not enough allowance");

            self.allowed.insert((from, sender), allowance - amount);
            self.transfer_from_to(from, to, amount)
        }

        fn transfer_from_to(&mut self, from: AccountId, to: AccountId, amount: u128) -> bool {
            assert!(self.balances.contains_key(&from), "Sender does not have a balance");
            let balance = self.balances.entry(from).or_insert(0);
            assert!(*balance >= amount, "Sender does not have a balance");
        
            *balance -= amount;
        
            let to_balance = self.balances.entry(to).or_insert(0);
            *to_balance += amount;
        
            self.env().emit_event(Transfer {
                from,
                to,
                amount,
            });
            true
        }
        

        #[ink(message)]
        pub fn mint(&mut self, amount: u128) {
            let sender = self.env().caller();
            let collateral_amount = amount * self.collateral_price / 100; // Collateral amount calculated based on the price feed

            self.collateral_pool += collateral_amount;
            assert!(self.collateral_pool > 0, "Collateral pool should be greater than 0");

            let balance = self.balances.entry(sender).or_insert(0);
            *balance += amount;
            self.total_supply += amount;

            self.env().emit_event(Minted {
                from: sender,
                to: sender,
                amount,
            });
        }

        #[ink(message)]
        pub fn redeem(&mut self, amount: u128) {
            let sender = self.env().caller();

            let balance = self.balances.entry(sender).or_insert(0);
            assert!(*balance >= amount, "Not enough balance to redeem");

            let collateral_amount = amount * self.collateral_price / 100; // Collateral amount calculated based on the price feed

            assert!(self.collateral_pool >= collateral_amount, "Not enough collateral in the pool");

            *balance -= amount;
            self.total_supply -= amount;
            self.collateral_pool -= collateral_amount;

            self.env().emit_event(Redeemed {
                from: sender,
                to: sender,
                amount,
            });
        }
    }

    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Minted {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Redeemed {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn create_contract_works() {
            let accounts =ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            assert_eq!(chest.name(), "Chest".to_string());
            assert_eq!(chest.symbol(), "CHEST".to_string());
            assert_eq!(chest.decimals(), 18);
            assert_eq!(chest.total_supply(), 0);
        }

        #[test]
        fn mint_works() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mut chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);
            assert_eq!(chest.total_supply(), amount);
            assert_eq!(chest.balance_of(accounts.alice), amount);
        }

        #[test]
        fn redeem_works() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mut chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);
            chest.redeem(amount / 2);
            assert_eq!(chest.total_supply(), amount / 2);
            assert_eq!(chest.balance_of(accounts.alice), amount / 2);
        }

        #[test]
        #[should_panic(expected = "Not enough balance to redeem")]
        fn redeem_not_enough_balance() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mut chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);
            chest.redeem(amount * 2);
        }

        #[test]
        #[should_panic(expected = "Not enough collateral in the pool")]
        fn redeem_not_enough_collateral() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mut chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);
            chest.redeem(amount);
            chest.redeem(amount);
        }

        #[test]
        fn transfer_works() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mut chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);

            // Transfer to Bob
            chest.transfer(accounts.bob, amount / 2);
            assert_eq!(chest.balance_of(accounts.alice), amount / 2);
            assert_eq!(chest.balance_of(accounts.bob), amount / 2);

            // Transfer from Bob to Charlie
            chest.approve(accounts.bob, amount / 4);
            chest.transfer_from(accounts.bob, accounts.charlie, amount / 4);
            assert_eq!(chest.balance_of(accounts.bob), amount / 4);
            assert_eq!(chest.balance_of(accounts.charlie), amount / 4);
        }

        #[test]
        #[should_panic(expected = "Not enough allowance")]
        fn transfer_not_enough_allowance() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mut chest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);

            chest.transfer(accounts.bob, amount / 2);
            chest.approve(accounts.bob, amount / 4);
            chest.transfer_from(accounts.bob, accounts.charlie, amount / 2);
        }

        #[test]
        #[should_panic(expected = "Not enough balance")]
        fn transfer_not_enough_balance() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().expect("Cannot get accounts");
            let mutchest = Chest::new("Chest".to_string(), "CHEST".to_string(), 18, accounts.alice, 100);
            let amount = 100_000;
            chest.mint(amount);

            chest.transfer(accounts.bob, amount * 2);
        }
    }
}



