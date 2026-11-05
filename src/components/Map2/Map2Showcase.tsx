import React, { useEffect, useRef, useState, useMemo, useCallback } from 'react';
import { Canvas } from '@react-three/fiber';
import * as THREE from 'three';
import { Map2Terrain } from './Map2Terrain';
import { Map2Atmosphere } from './Map2Atmosphere';
import { CelestialState } from './utils/map2AtmosphereMath';
import { Map2Water } from './Map2Water';
import { Map2Vegetation } from './Map2Vegetation';
import { Map2FantasyCrystals } from './Map2FantasyCrystals';
import { Map2Wildlife } from './Map2Wildlife';
import { getMap2Height } from './utils/map2TerrainGen';
import { Map2Biome } from './utils/map2BiomeConfig';
import { CameraMode } from './utils/map2Physics';
import { Map2CameraRig, Map2CameraView } from './Map2CameraRig';
import { Map2HUD } from './Map2HUD';

export interface Map2ShowcaseProps {
  isStandalone?: boolean;
  initialTimeOfDay?: number;
  initialTimeSpeed?: number;
  seaLevel?: number;
  quality?: 'high' | 'low';
  showVegetation?: boolean;
  showCrystals?: boolean;
  showWildlife?: boolean;
  initialCameraMode?: CameraMode;
}

interface SceneContentProps {
  timeOfDay: number;
  timeSpeed: number;
  seaLevel: number;
  quality: 'high' | 'low';
  showVegetation: boolean;
  showCrystals: boolean;
  showWildlife: boolean;
  cameraMode: CameraMode;
  onCameraModeChange: (mode: CameraMode) => void;
  viewRef: React.MutableRefObject<Map2CameraView>;
  teleportRef: React.MutableRefObject<{ x: number; z: number } | null>;
  onCelestialUpdate: (state: CelestialState) => void;
}

/**
 * Scene content mounting Map 2 Procedural Terrain, Atmospheric Sky, Gerstner Water,
 * GPU Instanced Vegetation, Fantasy Crystals, Autonomous Wildlife, and 4-Mode Camera Rig.
 */
const Map2SceneContent: React.FC<SceneContentProps> = ({
  timeOfDay,
  timeSpeed,
  seaLevel,
  quality,
  showVegetation,
  showCrystals,
  showWildlife,
  cameraMode,
  onCameraModeChange,
  viewRef,
  teleportRef,
  onCelestialUpdate,
}) => {
  const spawnHeight = useMemo(() => getMap2Height(0, 0), []);
  const [currentSunDir, setCurrentSunDir] = useState<THREE.Vector3>(() => new THREE.Vector3(120, 180, 80).normalize());
  const [currentSunColor, setCurrentSunColor] = useState<THREE.Color>(() => new THREE.Color('#fff7ed'));
  const [currentHorizonColor, setCurrentHorizonColor] = useState<THREE.Color>(() => new THREE.Color('#bae6fd'));
  const [currentWindSpeed, setCurrentWindSpeed] = useState<number>(1.2);

  const handleCelestialUpdate = useCallback((state: CelestialState) => {
    setCurrentSunDir(state.sunDirection);
    setCurrentSunColor(state.sunColor);
    setCurrentHorizonColor(state.skyHorizonColor);
    setCurrentWindSpeed(state.windSpeed);
    onCelestialUpdate(state);
  }, [onCelestialUpdate]);

  return (
    <>
      {/* Dynamic 24-Hour Atmospheric Sky Dome, Sun/Moon Trajectories, PBR Lighting & Fog */}
      <Map2Atmosphere
        timeOfDay={timeOfDay}
        timeSpeed={timeSpeed}
        onCelestialUpdate={handleCelestialUpdate}
      />

      {/* 4-Mode Camera Rig (Orbit, Fly, Walk/FPS with collision, and Cine Catmull-Rom Spline) */}
      <Map2CameraRig
        mode={cameraMode}
        onModeChange={onCameraModeChange}
        viewRef={viewRef}
        teleportRef={teleportRef}
        worldSize={200}
      />

      {/* Map 2 Root Group: Procedural Terrain, Water, Vegetation, Crystals & Wildlife */}
      <group name="map2-root-group">
        {/* Procedural Multioctave Ecotone Terrain */}
        <Map2Terrain
          size={200}
          segments={128}
          skirtDepth={15}
          sunDirection={currentSunDir}
        />

        {/* Realistic Multi-Octave Gerstner Wave Water Surface */}
        <Map2Water
          seaLevel={seaLevel}
          size={240}
          segments={128}
          sunDirection={currentSunDir}
          sunColor={currentSunColor}
          horizonHazeColor={currentHorizonColor}
          showWaterfalls={true}
        />

        {/* GPU Instanced Flora with Vertex Shader Wind Sway (10,000+ Instances) */}
        {showVegetation && (
          <Map2Vegetation
            size={200}
            waterLevel={seaLevel}
            windSpeed={currentWindSpeed}
            windDirection={0.785}
            spawnClearanceRadius={6.0}
            quality={quality}
          />
        )}

        {/* Bioluminescent Fantasy Crystal Spires & Subterranean Grottoes */}
        {showCrystals && (
          <Map2FantasyCrystals
            size={200}
            waterLevel={seaLevel}
            pulseSpeed={1.0}
            showPointLights={true}
          />
        )}

        {/* Autonomous Multi-Tier Wildlife Simulation (Boids Birds, Shoaling Fish & Terrestrial Fauna) */}
        {showWildlife && (
          <Map2Wildlife
            size={200}
            waterLevel={seaLevel}
            quality={quality}
          />
        )}

        {/* Central landmark beacon at spawn point */}
        <mesh position={[0, spawnHeight + 5, 0]} castShadow receiveShadow>
          <cylinderGeometry args={[1.5, 2.5, 10, 16]} />
          <meshStandardMaterial color="#0284c7" roughness={0.3} metalness={0.8} />
        </mesh>
      </group>
    </>
  );
};

/**
 * Primary container for Map 2 Open-World Showcase.
 * Supports both standalone fullscreen access (`map2.html`) and in-app dashboard integration (`App.tsx`).
 */
export const Map2Showcase: React.FC<Map2ShowcaseProps> = ({
  isStandalone = false,
  initialTimeOfDay = 11.5,
  initialTimeSpeed = 0.0,
  seaLevel = 12.0,
  quality = 'high',
  showVegetation = true,
  showCrystals = true,
  showWildlife = true,
  initialCameraMode = 'orbit',
}) => {
  const [timeOfDay, setTimeOfDay] = useState<number>(initialTimeOfDay);
  const [timeSpeed, setTimeSpeed] = useState<number>(initialTimeSpeed);
  const [isPlayingTime, setIsPlayingTime] = useState<boolean>(initialTimeSpeed > 0);
  const [cameraMode, setCameraMode] = useState<CameraMode>(initialCameraMode);
  const [vegEnabled, setVegEnabled] = useState<boolean>(showVegetation);
  const [crystEnabled, setCrystEnabled] = useState<boolean>(showCrystals);
  const [wildlifeEnabled, setWildlifeEnabled] = useState<boolean>(showWildlife);

  const viewRef = useRef<Map2CameraView>({
    targetX: 0,
    targetZ: 0,
    camX: 0,
    camY: 45,
    camZ: 90,
    bearing: 0,
    pitch: 0,
    biome: Map2Biome.TEMPERATE_PLAINS,
    fps: 60,
    locked: false,
    mode: initialCameraMode,
    speed: 0,
    isGrounded: false,
  });

  const teleportRef = useRef<{ x: number; z: number } | null>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const rendererRef = useRef<{ dispose?: () => void; forceContextLoss?: () => void } | null>(null);

  const handleCelestialCallback = useCallback((_state: CelestialState) => {
    // Atmosphere updates received
  }, []);

  const handleTeleport = useCallback((x: number, z: number) => {
    teleportRef.current = { x, z };
  }, []);

  const toggleDayNightPlay = useCallback(() => {
    if (isPlayingTime) {
      setTimeSpeed(0.0);
      setIsPlayingTime(false);
    } else {
      setTimeSpeed(0.5); // 0.5 hours per real second
      setIsPlayingTime(true);
    }
  }, [isPlayingTime]);

  // Explicit resource disposal and WebGL cleanup on unmount
  useEffect(() => {
    return () => {
      const scene = sceneRef.current;
      if (scene) {
        scene.traverse((obj) => {
          if (obj instanceof THREE.Mesh || obj instanceof THREE.Points || obj instanceof THREE.InstancedMesh) {
            if (obj.geometry) {
              obj.geometry.dispose();
            }
            if (obj.material) {
              if (Array.isArray(obj.material)) {
                obj.material.forEach((mat) => mat.dispose());
              } else {
                obj.material.dispose();
              }
            }
          }
        });
        scene.clear();
      }

      const renderer = rendererRef.current;
      if (renderer) {
        if (typeof renderer.dispose === 'function') {
          renderer.dispose();
        }
        if (typeof renderer.forceContextLoss === 'function') {
          renderer.forceContextLoss();
        }
      }
    };
  }, []);

  return (
    <div
      data-testid="map2-showcase"
      style={{
        width: '100%',
        height: '100%',
        position: 'relative',
        overflow: 'hidden',
        background: '#0a0f1d',
      }}
    >
      {/* 3D WebGL Canvas Viewport */}
      <Canvas
        shadows="percentage"
        dpr={[1, 1.5]}
        gl={{
          powerPreference: 'high-performance',
          antialias: true,
          toneMapping: THREE.ACESFilmicToneMapping,
          toneMappingExposure: 1.15,
        }}
        camera={{
          position: [0, 45, 90],
          fov: 55,
          near: 0.5,
          far: 3000,
        }}
        style={{ width: '100%', height: '100%' }}
        onCreated={(state) => {
          state.gl.toneMapping = THREE.ACESFilmicToneMapping;
          state.gl.toneMappingExposure = 1.15;
          sceneRef.current = state.scene;
          rendererRef.current = state.gl as { dispose?: () => void; forceContextLoss?: () => void };
        }}
      >
        <Map2SceneContent
          timeOfDay={timeOfDay}
          timeSpeed={timeSpeed}
          seaLevel={seaLevel}
          quality={quality}
          showVegetation={vegEnabled}
          showCrystals={crystEnabled}
          showWildlife={wildlifeEnabled}
          cameraMode={cameraMode}
          onCameraModeChange={setCameraMode}
          viewRef={viewRef}
          teleportRef={teleportRef}
          onCelestialUpdate={handleCelestialCallback}
        />
      </Canvas>

      {/* Interactive Explorer HUD Overlay (Biome Banner, Compass Ribbon, Minimap & Control Drawer) */}
      <Map2HUD
        viewRef={viewRef}
        cameraMode={cameraMode}
        onCameraModeChange={setCameraMode}
        timeOfDay={timeOfDay}
        onTimeOfDayChange={setTimeOfDay}
        isPlayingTime={isPlayingTime}
        onTogglePlayTime={toggleDayNightPlay}
        seaLevel={seaLevel}
        onTeleport={handleTeleport}
        showVegetation={vegEnabled}
        onToggleVegetation={() => setVegEnabled(!vegEnabled)}
        showCrystals={crystEnabled}
        onToggleCrystals={() => setCrystEnabled(!crystEnabled)}
        showWildlife={wildlifeEnabled}
        onToggleWildlife={() => setWildlifeEnabled(!wildlifeEnabled)}
        isStandalone={isStandalone}
        quality={quality}
        worldSize={200}
      />
    </div>
  );
};

export default Map2Showcase;
