import BN__default from 'bn.js';
import { LaunchpadPool } from '../layout.js';
import Decimal from 'decimal.js';
import '../../../marshmallow/index.js';
import '@solana/web3.js';
import '../../../marshmallow/buffer-layout.js';

interface PoolBaseAmount {
    virtualA: BN__default;
    virtualB: BN__default;
    realA: BN__default;
    realB: BN__default;
}
declare class CurveBase {
    static getPoolInitPriceByPool({ poolInfo, decimalA, decimalB, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode> | PoolBaseAmount;
        decimalA: number;
        decimalB: number;
    }): Decimal;
    static getPoolInitPriceByInit({ a, b, decimalA, decimalB, }: {
        a: BN__default;
        b: BN__default;
        decimalA: number;
        decimalB: number;
    }): Decimal;
    static getPoolPrice({ poolInfo, decimalA, decimalB, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode> | {
            virtualA: BN__default;
            virtualB: BN__default;
            realA: BN__default;
            realB: BN__default;
        };
        decimalA: number;
        decimalB: number;
    }): Decimal;
    static getPoolEndPrice({ supply, totalSell, totalLockedAmount, totalFundRaising, migrateFee, decimalA, decimalB, }: {
        supply: BN__default;
        totalSell: BN__default;
        totalLockedAmount: BN__default;
        totalFundRaising: BN__default;
        migrateFee: BN__default;
        decimalA: number;
        decimalB: number;
    }): Decimal;
    static getPoolEndPriceReal({ poolInfo, decimalA, decimalB, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode>;
        decimalA: number;
        decimalB: number;
    }): Decimal;
    static getInitParam({ supply, totalFundRaising, totalSell, totalLockedAmount, migrateFee, }: {
        supply: BN__default;
        totalSell: BN__default;
        totalLockedAmount: BN__default;
        totalFundRaising: BN__default;
        migrateFee: BN__default;
    }): {
        a: BN__default;
        b: BN__default;
        c: BN__default;
    };
    static buyExactIn({ poolInfo, amount, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode> | PoolBaseAmount;
        amount: BN__default;
    }): BN__default;
    static buyExactOut({ poolInfo, amount, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode> | PoolBaseAmount;
        amount: BN__default;
    }): BN__default;
    static sellExactIn({ poolInfo, amount, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode> | PoolBaseAmount;
        amount: BN__default;
    }): BN__default;
    static sellExactOut({ poolInfo, amount, }: {
        poolInfo: ReturnType<typeof LaunchpadPool.decode> | PoolBaseAmount;
        amount: BN__default;
    }): BN__default;
}

export { CurveBase, PoolBaseAmount };
