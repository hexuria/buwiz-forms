import seal from "./assets/1701-seal.png";
import barcodePageOne from "./assets/1701-barcode-page-1.png";
import barcodePageTwo from "./assets/1701-barcode-page-2.png";
import barcodePageThree from "./assets/1701-barcode-page-3.png";
import barcodePageFour from "./assets/1701-barcode-page-4.png";

/** Reviewed discrete crops from the pinned January 2018 official PDF raster. */
export const OFFICIAL_1701_SEAL = seal;
export const OFFICIAL_1701_BARCODES = [
  barcodePageOne,
  barcodePageTwo,
  barcodePageThree,
  barcodePageFour
] as const;
