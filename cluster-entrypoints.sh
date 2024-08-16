#!/bin/bash

curl https://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" -d '
  {
    "jsonrpc": "2.0", "id": 1,
    "method": "getClusterNodes"
  }
'
