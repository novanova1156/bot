import BN__default from 'bn.js';
import { RoundDirection, TradingTokenResult } from './calculator.js';

declare class ConstantProductCurve {
    static swapBaseInputWithoutFees(inputAmount: BN__default, inputVaultAmount: BN__default, onputVaultAmount: BN__default): BN__default;
    static swapBaseOutputWithoutFees(outputAmount: BN__default, inputVaultAmount: BN__default, onputVaultAmount: BN__default): BN__default;
    static lpTokensToTradingTokens(lpTokenAmount: BN__default, lpTokenSupply: BN__default, swapTokenAmount0: BN__default, swapTokenAmount1: BN__default, roundDirection: RoundDirection): TradingTokenResult;
}

export { ConstantProductCurve };
