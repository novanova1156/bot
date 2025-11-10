import BN__default from 'bn.js';

declare class CpmmFee {
    static tradingFee(amount: BN__default, tradeFeeRate: BN__default): BN__default;
    static protocolFee(amount: BN__default, protocolFeeRate: BN__default): BN__default;
    static fundFee(amount: BN__default, fundFeeRate: BN__default): BN__default;
    static creatorFee(amount: BN__default, creatorFeeRate: BN__default): BN__default;
    static splitCreatorFee(totalFee: BN__default, tradeFeeRate: BN__default, creatorFeeRate: BN__default): BN__default;
    static calculatePreFeeAmount(postFeeAmount: BN__default, tradeFeeRate: BN__default): BN__default;
}

export { CpmmFee };
