near-lending
=============
Overview
=====================

* The near-lending contract allows creating a lending pool.
* The lender will deposit lending tokens into the pool to conduct lending. Interest paid to lenders will depend on the amount of tokens borrowed by borrowers and is paid by lending tokens. Interest is calculated daily.
* The borrowers can mortgage collateral token to borrow lending token from the pool. The price data of the token pair currently we are getting from our oracle contract. Borrowers can borrow up to 50% of the collateral value. If the collateral price falls causing the loan to reach the liquidation threshold, someone else can liquidate the borrower's loan and receive an additional 5% incentive.

Using this contract
=====================

### Build
```bash
./build.sh
```
### Deploy
```bash
./dev-deploy.sh
```
Behind the scenes, this is creating an account and deploying a contract to it. On the console, notice a message like:

>Done deploying to dev-1234567890123

### Set enviroment variable:

```bash
ID=dev-1234567890123
```

```bash
LENDING_TOKEN=yourLendingTokenAccountId
```

```bash
BORROWING_TOKEN=yourBorrowingTokenAccountId
```

```bash
OWNER=yourContractOwner
```

```bash
LENDER=lenderAccount
```

```bash
BORROWER=borrowerAccount
```

### Initialize contract

```bash
near call $ID new --accountId $OWNER
```
### Create a new lending pool

Since it is quite laborious to get the token data via the promise, you must pass the decimals of the lending token as the function argument, ex: 18.
```bash
near call $ID create_new_lending_pool '{"lending_token": '$LENDING_TOKEN', "decimals": 18, "interest_rate": 2000}' --accountId $OWNER
```
```bash
near call $LENDING_TOKEN storage_deposit '{"account_id": '$ID'}' --accountId $OWNER --deposit 0.125
```
```bash
near call $ID create_new_lending_pool '{"lending_token": '$BORROWING_TOKEN', "decimals": 18, "interest_rate": 2000}' --accountId $OWNER
```
```bash
near call $BORROWING_TOKEN storage_deposit '{"account_id": '$ID'}' --accountId $OWNER --deposit 0.125
```

### Deposit lending token

Token prices are setted in contract oracle, you can check in repo simple-oracle. When user call function borrow, contract will check price and transfer token to borrower

```bash
near call $LENDING_TOKEN ft_transfer_call '{ "receiver_id": "'$ID'", "amount": "1000000000000000000000000", "msg": "{\"transfer_type\": \"Deposit\", \"token\": \"'$LENDING_TOKEN'\", \"pool_id\": 0}"}'  --accountId $LENDER --depositYocto 1
```

```bash
near call $BORROWING_TOKEN ft_transfer_call '{ "receiver_id": "'$ID'", "amount": "1000000000000000000000000", "msg": "{\"transfer_type\": \"Deposit\", \"token\": \"'$BORROWING_TOKEN'\", \"pool_id\": 1}"}'  --accountId $BORROWER --depositYocto 1
```

### Borrow lending token from pool

```bash
near call $ID borrow '{ "pool_id": 0, "amount": "1000000000"}' --accountId $BORROWER --depositYocto 1
```

### Withdraw token from lending pool
```bash
near call $ID withdraw '{"pool_id": 1, "amount": "1000000000"}' --accountId $LENDER --depositYocto 1
```
