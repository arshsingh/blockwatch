## Blockwatch

An ethereum event listener with webhook notifications.

### Configuration

Blockwatch will by default look for a `blockwatch.toml` file in the current working dir:

```toml
# can be an sqlite or postgres url
database_url = "sqlite:db.sqlite?mode=rwc"

# add a network called "main"
[networks.main]
chain_id = 1
rpc_url = "http://mainnet.infura.io/v3/..."
block_time = 12 

[networks.main.hooks.nft]
contracts = ["0x23581767a106ae21c074b2276D25e5C3e136a68b"]
url = "https://some-service.dev/handle-events"

[networks.main.hooks.another-hook]
contracts = ["0x123..", "0x456.."]
url = "https://another-service.dev/handle-events"

# [networks.another-network]
# ...
```

A custom location for the config file can be specified by passing it as an argument to the command:

```sh
blockwatch ./config/blockwatch.toml
```

#### ENV vars

Configuration can also be provided via environment variables instead:

```env
DATABASE_URL=postgres://pg:password@db:5432/blockwatch

NETWORKS__MAIN__CHAIN_ID=1
NETWORKS__MAIN__RPC_URL=http://mainnet.infura.io/v3/...
NETWORKS__MAIN__BLOCK_TIME=12

NETWORKS__MAIN__HOOKS__NFT__CONTRACTS=[0x23581767a106ae21c074b2276D25e5C3e136a68b]
NETWORKS__MAIN__HOOKS__NFT__URL=https://some-service.dev/handle-events
```

### Webhook failures

The webhook must respond with a 2xx status in 5 seconds for the delivery to be
considered successful. In case the server returns an error status, or fails to
respond in time, the delivery is marked as failed and stored in the `deliveries`
table. It will not be retried

### Deploying with docker-compose

```yaml
version: "3"
services:
  blockwatch:
    image: ghcr.io/arshsingh/blockwatch:0.1.0
    volumes:
      - ./blockwatch.toml:/blockwatch/blockwatch.toml
      - ./db.sqlite:/blockwatch/db.sqlite
```
