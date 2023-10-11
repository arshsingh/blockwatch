create table trackers (
    chain_id integer primary key,
    last_block_number integer not null,
    last_block_processed_at timestamp not null default current_timestamp
);

create table deliveries (
    id text primary key,
    chain_id integer not null,
    hook_id text not null,
    block_number integer not null,
    logs text not null,
    failed_at timestamp,
    created_at timestamp not null default current_timestamp,

    unique (hook_id, block_number)
); 
