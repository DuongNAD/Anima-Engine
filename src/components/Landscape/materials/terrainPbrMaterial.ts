import * as THREE from 'three';
import type { TerrainPbrArrays } from '../assets/worldAssets';

// prototype, to be replaced — W3-lite 6-layer biome/slope/height PBR splat terrain material with seamless procedural fallback.

export interface TerrainPbrMaterialParams {
  colorMap: THREE.Texture;
  normalMap: THREE.Texture;
  roughnessMap: THREE.Texture;
  detailMap: THREE.Texture;
  riverMaskMap: THREE.Texture;
  splatMap: THREE.Texture;
  heightUnits: number;
}

export interface TerrainPbrShaderHandle {
  applyPbrArrays(arrays: TerrainPbrArrays | null): void;
  updateTime(timeSec: number): void;
  dispose(): void;
}

function makeDummyLayerArray(r: number, g: number, b: number, a: number): THREE.DataArrayTexture {
  const layerCount = 6;
  const data = new Uint8Array(layerCount * 4);
  for (let i = 0; i < layerCount; i++) {
    const o = i * 4;
    data[o] = r;
    data[o + 1] = g;
    data[o + 2] = b;
    data[o + 3] = a;
  }
  const tex = new THREE.DataArrayTexture(data, 1, 1, layerCount);
  tex.format = THREE.RGBAFormat;
  tex.type = THREE.UnsignedByteType;
  tex.colorSpace = THREE.NoColorSpace;
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.generateMipmaps = false;
  tex.needsUpdate = true;
  return tex;
}

export function createTerrainPbrMaterial(
  params: TerrainPbrMaterialParams,
): { material: THREE.MeshStandardMaterial; handle: TerrainPbrShaderHandle } {
  const dummyAlbedo = makeDummyLayerArray(180, 180, 180, 255);
  const dummyNormal = makeDummyLayerArray(128, 128, 255, 255);

  const uniforms: {
    uDetail: { value: THREE.Texture };
    uRiverMask: { value: THREE.Texture };
    uSplatMap: { value: THREE.Texture };
    uTime: { value: number };
    uPbrEnabled: { value: number };
    uHeightUnits: { value: number };
    uPbrAlbedo: { value: THREE.DataArrayTexture };
    uPbrNormal: { value: THREE.DataArrayTexture };
  } = {
    uDetail: { value: params.detailMap },
    uRiverMask: { value: params.riverMaskMap },
    uSplatMap: { value: params.splatMap },
    uTime: { value: 0 },
    uPbrEnabled: { value: 0 },
    uHeightUnits: { value: Math.max(1, params.heightUnits) },
    uPbrAlbedo: { value: dummyAlbedo },
    uPbrNormal: { value: dummyNormal },
  };

  const material = new THREE.MeshStandardMaterial({
    map: params.colorMap,
    normalMap: params.normalMap,
    normalScale: new THREE.Vector2(1, 1),
    roughnessMap: params.roughnessMap,
    roughness: 1.0,
    metalness: 0.0,
  });

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uDetail = uniforms.uDetail;
    shader.uniforms.uRiverMask = uniforms.uRiverMask;
    shader.uniforms.uSplatMap = uniforms.uSplatMap;
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uPbrEnabled = uniforms.uPbrEnabled;
    shader.uniforms.uHeightUnits = uniforms.uHeightUnits;
    shader.uniforms.uPbrAlbedo = uniforms.uPbrAlbedo;
    shader.uniforms.uPbrNormal = uniforms.uPbrNormal;

    shader.vertexShader =
      'varying vec3 vW3WorldPos;\nvarying vec3 vW3WorldNormal;\n' +
      shader.vertexShader.replace(
        '#include <worldpos_vertex>',
        `#include <worldpos_vertex>
        vec4 w3WorldPos4 = modelMatrix * vec4( transformed, 1.0 );
        vW3WorldPos = w3WorldPos4.xyz;
        vW3WorldNormal = normalize( mat3( modelMatrix ) * objectNormal );`,
      );

    shader.fragmentShader =
      `precision highp sampler2DArray;
uniform sampler2D uDetail;
uniform sampler2D uRiverMask;
uniform sampler2D uSplatMap;
uniform float uTime;
uniform float uPbrEnabled;
uniform float uHeightUnits;
uniform sampler2DArray uPbrAlbedo;
uniform sampler2DArray uPbrNormal;
varying vec3 vW3WorldPos;
varying vec3 vW3WorldNormal;

vec4 w3SampleAntiTile(sampler2DArray texArr, vec2 wp, float layer) {
  vec2 uvA = wp * 0.085;
  vec2 rotWp = vec2(wp.x * 0.7986 - wp.y * 0.6018, wp.x * 0.6018 + wp.y * 0.7986);
  vec2 uvB = rotWp * 0.021 + vec2(0.37, 0.71);
  vec4 a = texture(texArr, vec3(uvA, layer));
  vec4 b = texture(texArr, vec3(uvB, layer));
  return mix(a, b, 0.44);
}

vec4 w3SampleTriplanarRock(sampler2DArray texArr, vec3 wp, vec3 wn) {
  vec3 blend = abs(wn);
  blend = max(blend - vec3(0.2), vec3(0.001));
  blend /= (blend.x + blend.y + blend.z);
  vec4 cx = w3SampleAntiTile(texArr, wp.zy, 2.0);
  vec4 cy = w3SampleAntiTile(texArr, wp.xz, 2.0);
  vec4 cz = w3SampleAntiTile(texArr, wp.xy, 2.0);
  return cx * blend.x + cy * blend.y + cz * blend.z;
}

void w3ComputeLayerWeights(
  vec2 mapUv,
  out float wGrass,
  out float wForest,
  out float wRock,
  out float wSand,
  out float wSnow,
  out float wMud
) {
  vec4 spl = texture2D(uSplatMap, mapUv);
  vec3 wn = normalize(vW3WorldNormal);
  float slopeAmt = 1.0 - clamp(wn.y, 0.0, 1.0);
  wRock = smoothstep(0.16, 0.44, slopeAmt);

  float hNorm = clamp(vW3WorldPos.y / max(uHeightUnits, 1.0), 0.0, 1.0);
  float snowNoise = texture2D(uDetail, vW3WorldPos.xz * 0.012).r - 0.5;
  float snowHeight = smoothstep(0.62, 0.80, hNorm + snowNoise * 0.14) * (1.0 - wRock * 0.9);
  wSnow = max(spl.a * (1.0 - wRock * 0.85), snowHeight);

  float rem = max(0.0, 1.0 - wRock - wSnow);
  float specSum = max(0.001, spl.r + spl.g + spl.b);
  float specScale = min(1.0, 1.0 / max(1.0, specSum));
  wForest = spl.r * specScale * rem;
  wSand = spl.g * specScale * rem;
  wMud = spl.b * specScale * rem;
  wGrass = max(0.0, rem - wForest - wSand - wMud);

  float total = max(0.001, wGrass + wForest + wRock + wSand + wSnow + wMud);
  wGrass /= total;
  wForest /= total;
  wRock /= total;
  wSand /= total;
  wSnow /= total;
  wMud /= total;
}
` +
      shader.fragmentShader
        .replace(
          '#include <map_fragment>',
          `#include <map_fragment>
          {
            float dtl = texture2D(uDetail, vMapUv * 220.0).r * 2.0;
            float riv = texture2D(uRiverMask, vMapUv).r;
            if (uPbrEnabled > 0.5) {
              float wG, wF, wR, wSa, wSn, wM;
              w3ComputeLayerWeights(vMapUv, wG, wF, wR, wSa, wSn, wM);
              vec3 wn = normalize(vW3WorldNormal);
              vec3 pbrSrgb = vec3(0.0);
              if (wG > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 0.0).rgb * wG;
              if (wF > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 1.0).rgb * wF;
              if (wR > 0.01) pbrSrgb += w3SampleTriplanarRock(uPbrAlbedo, vW3WorldPos, wn).rgb * wR;
              if (wSa > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 3.0).rgb * wSa;
              if (wSn > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 4.0).rgb * wSn;
              if (wM > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 5.0).rgb * wM;

              vec3 pbrLin = pow(max(pbrSrgb, vec3(0.001)), vec3(2.2));
              float pbrLuma = dot(pbrLin, vec3(0.299, 0.587, 0.114));
              vec3 pbrBlended = diffuseColor.rgb * mix(0.62, 1.48, clamp(pbrLuma * 1.45, 0.0, 1.0)) * 0.56 + pbrLin * 0.44;
              pbrBlended *= mix(1.0, dtl, 0.16);
              diffuseColor.rgb = mix(pbrBlended, diffuseColor.rgb, clamp(riv * 0.85, 0.0, 0.85));
            } else {
              diffuseColor.rgb *= mix(1.0, dtl, 0.34);
            }
            if (riv > 0.01) {
              float flow1 = texture2D(uDetail, vMapUv * 260.0 + vec2(uTime * 0.025, uTime * 0.045)).r * 2.0;
              float flow2 = texture2D(uDetail, vMapUv * 140.0 - vec2(uTime * 0.018, uTime * 0.03)).r * 2.0;
              diffuseColor.rgb *= mix(1.0, flow1 * 0.55 + flow2 * 0.45, riv * 0.3);
            }
          }`,
        )
        .replace(
          '#include <normal_fragment_maps>',
          `#include <normal_fragment_maps>
          {
            float distFade = (1.0 - smoothstep(140.0, 520.0, length(vViewPosition))) * uPbrEnabled;
            if (distFade > 0.01) {
              float wG, wF, wR, wSa, wSn, wM;
              w3ComputeLayerWeights(vMapUv, wG, wF, wR, wSa, wSn, wM);
              vec3 wn = normalize(vW3WorldNormal);
              vec3 pbrNs = vec3(0.0);
              if (wG > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 0.0).xyz * wG;
              if (wF > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 1.0).xyz * wF;
              if (wR > 0.01) pbrNs += w3SampleTriplanarRock(uPbrNormal, vW3WorldPos, wn).xyz * wR;
              if (wSa > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 3.0).xyz * wSa;
              if (wSn > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 4.0).xyz * wSn;
              if (wM > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 5.0).xyz * wM;
              vec3 microN = pbrNs * 2.0 - 1.0;
              mapN.xy += microN.xy * (0.65 * distFade);
              normal = normalize(tbn * mapN);
            }
          }`,
        );
  };

  material.customProgramCacheKey = () => 'w3-lite-terrain-pbr-v1';

  const handle: TerrainPbrShaderHandle = {
    applyPbrArrays(arrays) {
      if (arrays) {
        uniforms.uPbrAlbedo.value = arrays.albedoArray;
        uniforms.uPbrNormal.value = arrays.normalArray;
        uniforms.uPbrEnabled.value = 1;
      } else {
        uniforms.uPbrAlbedo.value = dummyAlbedo;
        uniforms.uPbrNormal.value = dummyNormal;
        uniforms.uPbrEnabled.value = 0;
      }
    },
    updateTime(timeSec) {
      uniforms.uTime.value = timeSec;
    },
    dispose() {
      dummyAlbedo.dispose();
      dummyNormal.dispose();
      material.dispose();
    },
  };

  return { material, handle };
}
