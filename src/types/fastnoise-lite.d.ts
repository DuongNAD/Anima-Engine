declare module 'fastnoise-lite' {
  export default class FastNoiseLite {
    constructor(seed?: number);
    static NoiseType: {
      OpenSimplex2: string;
      OpenSimplex2S: string;
      Cellular: string;
      Perlin: string;
      ValueCubic: string;
      Value: string;
    };
    static RotationType3D: any;
    static FractalType: any;
    static CellularDistanceFunction: any;
    static CellularReturnType: any;
    static DomainWarpType: any;

    SetNoiseType(noiseType: string | any): void;
    SetSeed(seed: number): void;
    SetFrequency(frequency: number): void;
    SetFractalType(fractalType: any): void;
    SetFractalOctaves(octaves: number): void;
    SetFractalLacunarity(lacunarity: number): void;
    SetFractalGain(gain: number): void;
    SetFractalWeightedStrength(weightedStrength: number): void;
    SetCellularDistanceFunction(cellularDistanceFunction: any): void;
    SetCellularReturnType(cellularReturnType: any): void;
    SetCellularJitter(cellularJitter: number): void;
    SetDomainWarpType(domainWarpType: any): void;
    SetDomainWarpAmp(domainWarpAmp: number): void;
    GetNoise(x: number, y: number, z?: number): number;
  }
}
