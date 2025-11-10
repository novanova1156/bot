import BN__default from 'bn.js';
import Decimal from 'decimal.js';
import { LaunchpadPool } from '../layout.js';
import { CurveBase, PoolBaseAmount } from './curveBase.js';
import '../../../marshmallow/index.js';
import '@solana/web3.js';
import '../../../marshmallow/buffer-layout.js';

declare class FixedPriceCurve extends CurveBase {
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
        totalFundRaising: BN__default;
        totalLockedAmount: BN__default;
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
    static getAmountOut({ amountIn, initInput, initOutput }: {
        amountIn: BN__default;
        initInput: BN__default;
        initOutput: BN__default;
    }): BN__default;
    static getAmountIn({ amountOut, initInput, initOutput }: {
        amountOut: BN__default;
        initInput: BN__default;
        initOutput: BN__default;
    }): BN__default;
}

export { FixedPriceCurve };
