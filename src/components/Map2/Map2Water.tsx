import React, { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import {
  createMap2WaterMaterial,
  createMap2WaterfallMaterial,
  Map2WaterShaderUniforms,
  Map2WaterfallShaderUniforms,
} from './utils/map2Shaders';
import {
  GerstnerWaveDefinition,
  MAP2_DEFAULT_GERSTNER_WAVES,
  MAP2_WATERFALL_LOCATIONS,
} from './utils/map2WaterMath';

export type { GerstnerWaveDefinition, WaterSampleResult } from './utils/map2WaterMath';

// -------------------------------------------------------------------------------------------------
// React Three Fiber Component: Map2Water
// -------------------------------------------------------------------------------------------------

export interface Map2WaterProps {
  seaLevel?: number; // Sea level height in meters (default: 12.0m)
  size?: number;     // Water plane world size (default: 240.0m)
  segments?: number; // Water plane mesh tessellation segments (default: 128)
  sunDirection?: THREE.Vector3;
  sunColor?: THREE.Color;
  horizonHazeColor?: THREE.Color;
  showWaterfalls?: boolean;
  waves?: GerstnerWaveDefinition[];
}

/**
 * Photorealistic Gerstner Wave Water Plane, Shoreline Foam & Waterfall Spray Curtains for Map 2.
 */
export const Map2Water: React.FC<Map2WaterProps> = ({
  seaLevel = 12.0,
  size = 240.0,
  segments = 128,
  sunDirection = new THREE.Vector3(120, 180, 80).normalize(),
  sunColor = new THREE.Color('#fff7ed'),
  horizonHazeColor = new THREE.Color('#bae6fd'),
  showWaterfalls = true,
  waves = MAP2_DEFAULT_GERSTNER_WAVES,
}) => {
  const waterMeshRef = useRef<THREE.Mesh>(null);
  const waterfallMeshesRef = useRef<THREE.Mesh[]>([]);

  // 1. Water Plane Geometry (oriented horizontally on XZ)
  const waterGeometry = useMemo(() => {
    const geo = new THREE.PlaneGeometry(size, size, segments, segments);
    geo.rotateX(-Math.PI / 2);
    return geo;
  }, [size, segments]);

  // 2. Water Shader Material with Beer-Lambert & Shoreline Foam
  const waterMaterial = useMemo(() => {
    const w0 = waves[0] || MAP2_DEFAULT_GERSTNER_WAVES[0];
    const w1 = waves[1] || MAP2_DEFAULT_GERSTNER_WAVES[1];
    const w2 = waves[2] || MAP2_DEFAULT_GERSTNER_WAVES[2];
    const w3 = waves[3] || MAP2_DEFAULT_GERSTNER_WAVES[3];

    return createMap2WaterMaterial({
      uSeaLevel: { value: seaLevel },
      uSunDir: { value: sunDirection },
      uSunColor: { value: sunColor },
      uHorizonHazeColor: { value: horizonHazeColor },
      uWave0: { value: new THREE.Vector4(w0.direction[0], w0.direction[1], w0.amplitude, w0.wavelength) },
      uWaveParams0: { value: new THREE.Vector2(w0.speed, w0.steepness) },
      uWave1: { value: new THREE.Vector4(w1.direction[0], w1.direction[1], w1.amplitude, w1.wavelength) },
      uWaveParams1: { value: new THREE.Vector2(w1.speed, w1.steepness) },
      uWave2: { value: new THREE.Vector4(w2.direction[0], w2.direction[1], w2.amplitude, w2.wavelength) },
      uWaveParams2: { value: new THREE.Vector2(w2.speed, w2.steepness) },
      uWave3: { value: new THREE.Vector4(w3.direction[0], w3.direction[1], w3.amplitude, w3.wavelength) },
      uWaveParams3: { value: new THREE.Vector2(w3.speed, w3.steepness) },
    });
  }, [seaLevel, sunDirection, sunColor, horizonHazeColor, waves]);

  // 3. Waterfall Curtains Geometries & Materials
  const waterfallData = useMemo(() => {
    return MAP2_WATERFALL_LOCATIONS.map((loc) => {
      const geo = new THREE.PlaneGeometry(loc.width, loc.height, 16, 32);
      const mat = createMap2WaterfallMaterial();
      return { geo, mat, ...loc };
    });
  }, []);

  // 4. Render Frame Animation Loop
  useFrame(({ clock }) => {
    const time = clock.getElapsedTime();

    // Update Water Uniforms
    if (waterMeshRef.current && waterMeshRef.current.material) {
      const mat = waterMeshRef.current.material as THREE.ShaderMaterial;
      const u = mat.uniforms as unknown as Map2WaterShaderUniforms;
      if (u) {
        u.uTime.value = time;
        u.uSeaLevel.value = seaLevel;
        u.uSunDir.value.copy(sunDirection);
        u.uSunColor.value.copy(sunColor);
        u.uHorizonHazeColor.value.copy(horizonHazeColor);
      }
    }

    // Update Waterfall Curtains Uniforms
    waterfallData.forEach(({ mat }) => {
      const u = mat.uniforms as unknown as Map2WaterfallShaderUniforms;
      if (u) {
        u.uTime.value = time;
      }
    });
  });

  // 5. Explicit WebGL Cleanup on Unmount
  useEffect(() => {
    return () => {
      waterGeometry.dispose();
      waterMaterial.dispose();
      waterfallData.forEach(({ geo, mat }) => {
        geo.dispose();
        mat.dispose();
      });
    };
  }, [waterGeometry, waterMaterial, waterfallData]);

  return (
    <group name="map2-water-system">
      {/* Primary Gerstner Wave Water Surface */}
      <mesh
        ref={waterMeshRef}
        name="map2-gerstner-water-mesh"
        geometry={waterGeometry}
        material={waterMaterial}
        receiveShadow
      />

      {/* River Cascades & Waterfall Foam Curtains */}
      {showWaterfalls && (
        <group name="map2-waterfalls-group">
          {waterfallData.map((item, idx) => (
            <mesh
              key={`map2-waterfall-${idx}`}
              ref={(el) => {
                if (el) waterfallMeshesRef.current[idx] = el;
              }}
              name={`map2-waterfall-${idx}`}
              geometry={item.geo}
              material={item.mat}
              position={item.position}
              rotation={[0, item.rotationY, 0]}
            />
          ))}
        </group>
      )}
    </group>
  );
};

export default Map2Water;
