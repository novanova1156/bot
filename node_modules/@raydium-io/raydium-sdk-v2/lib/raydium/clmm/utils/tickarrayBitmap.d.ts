import BN__default from 'bn.js';
import { TickArrayBitmapExtensionLayout } from '../layout.js';
import '../../../marshmallow/index.js';
import '@solana/web3.js';
import '../../../marshmallow/buffer-layout.js';

declare const EXTENSION_TICKARRAY_BITMAP_SIZE = 14;
declare class TickArrayBitmap {
    static maxTickInTickarrayBitmap(tickSpacing: number): number;
    static getBitmapTickBoundary(tickarrayStartIndex: number, tickSpacing: number): {
        minValue: number;
        maxValue: number;
    };
    static nextInitializedTickArrayStartIndex(bitMap: BN__default, lastTickArrayStartIndex: number, tickSpacing: number, zeroForOne: boolean): {
        isInit: boolean;
        tickIndex: number;
    };
}
declare class TickArrayBitmapExtensionUtils {
    static getBitmapOffset(tickIndex: number, tickSpacing: number): number;
    static getBitmap(tickIndex: number, tickSpacing: number, tickArrayBitmapExtension: ReturnType<typeof TickArrayBitmapExtensionLayout.decode>): {
        offset: number;
        tickarrayBitmap: BN__default[];
    };
    static checkExtensionBoundary(tickIndex: number, tickSpacing: number): void;
    static extensionTickBoundary(tickSpacing: number): {
        positiveTickBoundary: number;
        negativeTickBoundary: number;
    };
    static checkTickArrayIsInit(tickArrayStartIndex: number, tickSpacing: number, tickArrayBitmapExtension: ReturnType<typeof TickArrayBitmapExtensionLayout.decode>): {
        isInitialized: boolean;
        startIndex: number;
    };
    static nextInitializedTickArrayFromOneBitmap(lastTickArrayStartIndex: number, tickSpacing: number, zeroForOne: boolean, tickArrayBitmapExtension: ReturnType<typeof TickArrayBitmapExtensionLayout.decode>): {
        isInit: boolean;
        tickIndex: number;
    };
    static nextInitializedTickArrayInBitmap(tickarrayBitmap: BN__default[], nextTickArrayStartIndex: number, tickSpacing: number, zeroForOne: boolean): {
        isInit: boolean;
        tickIndex: number;
    };
    static tickArrayOffsetInBitmap(tickArrayStartIndex: number, tickSpacing: number): number;
}

export { EXTENSION_TICKARRAY_BITMAP_SIZE, TickArrayBitmap, TickArrayBitmapExtensionUtils };
