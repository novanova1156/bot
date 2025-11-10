import BN__default from 'bn.js';
import Decimal from 'decimal.js';

declare class MathLaunch {
    static _Q64: Decimal;
    static _multipler(decimals: number): Decimal;
    static getPrice({ priceX64, decimalA, decimalB }: {
        priceX64: BN__default;
        decimalA: number;
        decimalB: number;
    }): Decimal;
    static getPriceX64({ price, decimalA, decimalB }: {
        price: Decimal;
        decimalA: number;
        decimalB: number;
    }): BN__default;
}
declare function checkPoolToAmm({ supply, totalFundRaisingB, totalLockedAmount, totalSellA, migrateType, decimalsA }: {
    supply: BN__default;
    totalSellA: BN__default;
    totalLockedAmount: BN__default;
    totalFundRaisingB: BN__default;
    migrateType: 'amm' | 'cpmm';
    decimalsA: number;
}): boolean;

export { MathLaunch, checkPoolToAmm };
