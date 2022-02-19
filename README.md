near-lending
=============
Overview
=====================

* The near-lending contract allows creating a lending pool with a pair of lending-collateral tokens.
* The lender will deposit lending tokens into the pool to conduct lending. Interest paid to lenders will depend on the amount of tokens borrowed by borrowers and is paid by lending tokens. Interest is calculated daily.
* The borrowers can mortgage collateral token to borrow lending token from the pool. The price data of the token pair currently we are getting from Ref-finance, but in the future, we will use the price data from oracle. Borrowers can borrow up to 50% of the collateral value. If the collateral price falls causing the loan to reach the liquidation threshold, someone else can liquidate the borrower's loan and receive an additional 5% incentive.

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
OWNER=yourContractOwner
```

```bash
LENDER=lenderAccount
```

```bash
BORROWER=borrowerAccount
```

```bash
LIQUIDATOR=liquidatorAccount
```

### Initialize contract

```bash
near call $ID new --accountId $OWNER
```
### Create a new lending pool

Because the contract now gets price data from Ref-finance, the lending token and collateral token must have a pool with wnear in the Ref-finance testnet.

```bash
near call $ID create_new_lending_pool '{"lending_token": YOUR_LENDING_TOKEN, "collateral_token": YOUR_COLLATERAL_TOKEN, "ref_pool_ids": [pool id collateral - wnear, pool id lending - wnear], "interest_rate": 2000}' --accountId $OWNER
```

### Deposit lending token

```bash
near call YOUR_LENDING_TOKEN ft_transfer_call '{ "receiver_id": "$ID", "amount": "1000000000000000000000000", "msg": "{\"transfer_type\": \"Deposit\", \"token\": YOUR_COLLATERAL_TOKEN, \"pool_id\": 0}"}'  --accountId $LENDER --depositYocto 1
```
### Mortgage collateral token

```bash 
near call YOUR_COLLATERAL_TOKEN ft_transfer_call '{ "receiver_id": "'$ID'", "amount": "1000000000000000000000000", "msg": "{\"transfer_type\": \"Mortgage\", \"token\": YOUR_LENDING_TOKEN, \"pool_id\": 0}"}'  --accountId $BORROWER --depositYocto 1
```

### Borrow lending token from pool

```bash
near call $ID borrow '{ "pool_id": 0, "amount": "1000000000000000000"}' --accountId $BORROWER --depositYocto 1
```

### Liquidate collateral token

If the collateral price falls causing the loan to reach the liquidation threshold 65%, someone else can liquidate the borrower's loan
```bash
near call YOUR_LENDING_TOKEN ft_transfer_call '{ "receiver_id": "$ID", "amount": "1000000000000000000000000", "msg": "{\"transfer_type\": \"Liquidate\", \"token\": YOUR_COLLATERAL_TOKEN, \"pool_id\": 0}"}'  --accountId $LIQUIDATOR --depositYocto 1
```
