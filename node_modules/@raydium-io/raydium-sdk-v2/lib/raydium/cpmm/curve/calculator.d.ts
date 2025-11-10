import BN__default from 'bn.js';

declare enum RoundDirection {
    Floor = 0,
    Ceiling = 1
}
declare type SwapWithoutFeesResult = {
    destinationAmountSwapped: BN__default;
};
declare type TradingTokenResult = {
    tokenAmount0: BN__default;
    tokenAmount1: BN__default;
};
declare type SwapResult = {
    newInputVaultAmount: BN__default;
    newOutputVaultAmount: BN__default;
    inputAmount: BN__default;
    outputAmount: BN__default;
    tradeFee: BN__default;
    protocolFee: BN__default;
    fundFee: BN__default;
    creatorFee: BN__default;
};
declare enum TradeDirection {
    ZeroForOne = 0,
    OneForZero = 1
}
declare enum TradeDirectionOpposite {
    OneForZero = 0,
    ZeroForOne = 1
}
declare class CurveCalculator {
    static validate_supply(tokenAmount0: BN__default, tokenAmount1: BN__default): void;
    static swapBaseInput(inputAmount: BN__default, inputVaultAmount: BN__default, outputVaultAmount: BN__default, tradeFeeRate: BN__default, creatorFeeRate: BN__default, protocolFeeRate: BN__default, fundFeeRate: BN__default, isCreatorFeeOnInput: boolean): SwapResult;
    static swapBaseOutput(outputAmount: BN__default, inputVaultAmount: BN__default, outputVaultAmount: BN__default, tradeFeeRate: BN__default, creatorFeeRate: BN__default, protocolFeeRate: BN__default, fundFeeRate: BN__default, isCreatorFeeOnInput: boolean): SwapResult;
}

export { CurveCalculator, RoundDirection, SwapResult, SwapWithoutFeesResult, TradeDirection, TradeDirectionOpposite, TradingTokenResult };
