## Blockwatch

An ethereum event listener with webhook notifications.

### Configuration

Blockwatch will by default look for a `blockwatch.config.json` file in the current working dir.

```javascript
{
  // A postgres or sqlite database URL
  "database_url": "sqlite:db.sqlite?mode=rwc"

  "networks": {
    "main": {
      "chain_id": 1,
      "rpc_url": "http://mainnet.infura.io/v3/...",
      "block_time": 12,

      // optional. default: 2000 
      "logs_page_size": 2000
    },

    // Multiple networks can be added
    "optimism-sepolia": {
      "chain_id": 11155111,
      ...
    }
  },

  "hooks": {
    "nft": {
      "chain_id": 1,
      "contracts": ["0x23581767a106ae21c074b2276D25e5C3e136a68b"],
      "url": "https://some-service.dev/handle-events"

      // optional. default: 5
      "timeout": 10,
    },

    // Multiple hooks can be added, even for the same chain_id
    "another-hook": {
      "chain_id": 1,
      "contracts": ["0x123..", "0x456.."],
      "url": "https://another-service.dev/handle-events"
    }
  }
}
```
See [config.rs](src/config.rs) for some comments about the config values.

A custom location for the config file can be specified by passing it as an argument to the command:

```sh
blockwatch ./config/blockwatch.config.json
```

#### Remote config sources

Configuration can be loaded via a URL that responds with the config in JSON format. Multiple sources
for the configuration can also be used. The config will be merged in the same order they are specified:

```sh
blockwatch ./config/blockwatch.config.json https://some-service.dev/blockwatch.json
```

If the URL in the example above responds with the following JSON, it will add a new hook
to the configuration:
```javascript
{
  "hooks": {
    "new-hook": { ... },
  }
}
```

#### ENV vars

Environment variables are merged in the config after all other sources, so they can
be used to inject sensitive values:

```env
DATABASE_URL=postgres://pg:password@db:5432/blockwatch
NETWORKS__MAIN__RPC_URL=http://mainnet.infura.io/v3/...
```

You may also specify the entire config with env vars:

```env
DATABASE_URL=postgres://pg:password@db:5432/blockwatch

NETWORKS__MAIN__CHAIN_ID=1
NETWORKS__MAIN__RPC_URL=http://mainnet.infura.io/v3/...
NETWORKS__MAIN__BLOCK_TIME=12

HOOKS__NFT__CHAIN_ID=1
HOOKS__NFT__CONTRACTS=[0x23581767a106ae21c074b2276D25e5C3e136a68b]
HOOKS__NFT__URL=https://some-service.dev/handle-events
```

### Webhook failures

The webhook must respond with a 2xx status within the timeout period for the
delivery to be considered successful. In case the server returns an error status,
or fails to respond in time, the delivery is marked as failed and stored in
the `deliveries` table. It will not be retried

### Deploying with docker-compose

```yaml
version: "3"
services:
  blockwatch:
    image: ghcr.io/arshsingh/blockwatch
    volumes:
      - ./blockwatch.config.json:/blockwatch/blockwatch.config.json
      - ./db.sqlite:/blockwatch/db.sqlite
```
