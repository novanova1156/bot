import { AddressLookupTableAccount, Connection, PublicKey } from '@solana/web3.js';

interface CacheLTA {
    [key: string]: AddressLookupTableAccount;
}
declare function getMultipleLookupTableInfo({ connection, address, cluster, }: {
    connection: Connection;
    address: PublicKey[];
    cluster?: "mainnet" | "devnet";
}): Promise<CacheLTA>;
declare const LOOKUP_TABLE_CACHE: CacheLTA;
declare const getMainLookupTableCache: (connection: Connection) => Promise<CacheLTA>;
declare const DEV_LOOKUP_TABLE_CACHE: CacheLTA;
declare const getDevLookupTableCache: (connection: Connection) => Promise<CacheLTA>;

export { CacheLTA, DEV_LOOKUP_TABLE_CACHE, LOOKUP_TABLE_CACHE, getDevLookupTableCache, getMainLookupTableCache, getMultipleLookupTableInfo };
