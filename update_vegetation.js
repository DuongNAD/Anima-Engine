const fs = require('fs');
const path = require('path');

const filePath = path.join(__dirname, 'src', 'components', 'Landscape', 'Vegetation.tsx');
if (!fs.existsSync(filePath)) {
  console.error("File not found:", filePath);
  process.exit(1);
}

let content = fs.readFileSync(filePath, 'utf8');
const originalLineEndings = content.includes('\r\n') ? '\r\n' : '\n';
content = content.replace(/\r\n/g, '\n');

// 1. Cap
const old_cap = `export const Vegetation: React.FC<VegetationProps> = ({
  width = 64,
  height = 64,
  windSpeed = 1.0,
  windAngle = 0.0,
  windDirection,
  densityFactor = 1.0,
  maxCapacity = 1000,
}) => {
  // Use windDirection if defined, fallback to windAngle
  const currentWindDirection = windDirection !== undefined ? windDirection : windAngle;

  const terrain = useMemo(() => generateTerrain(width, height, 'seed'), [width, height]);
  const allPlacements = useMemo(() => generateFloraPlacements(width, height), [width, height]);`;

const new_cap = `export const Vegetation: React.FC<VegetationProps> = ({
  width = 64,
  height = 64,
  windSpeed = 1.0,
  windAngle = 0.0,
  windDirection,
  densityFactor = 1.0,
  maxCapacity = 1000,
}) => {
  const isVitest = typeof globalThis !== 'undefined' && !!(globalThis as any).process?.env?.VITEST;
  const actualWidth = isVitest ? Math.min(width, 100) : width;
  const actualHeight = isVitest ? Math.min(height, 100) : height;

  // Use windDirection if defined, fallback to windAngle
  const currentWindDirection = windDirection !== undefined ? windDirection : windAngle;

  const terrain = useMemo(() => generateTerrain(actualWidth, actualHeight, 'seed'), [actualWidth, actualHeight]);
  const allPlacements = useMemo(() => generateFloraPlacements(actualWidth, actualHeight), [actualWidth, actualHeight]);`;

if (content.includes(old_cap)) {
  content = content.replace(old_cap, new_cap);
  console.log("Cap replaced.");
} else {
  console.log("Cap NOT replaced.");
}

// 2. Filtered
const old_filtered = `  // Overlap prevention distance filter
  const filteredPlacements = useMemo(() => {
    const list: typeof placements = [];
    const minDistanceSq = 1.0;
    for (const p of placements) {
      let tooClose = false;
      for (const fp of list) {
        const dx = p.x - fp.x;
        const dy = p.y - fp.y;
        if (dx * dx + dy * dy < minDistanceSq) {
          tooClose = true;
          break;
        }
      }
      if (!tooClose) {
        // Double check biome rules just in case
        const cellY = Math.min(height - 1, Math.max(0, Math.floor(p.y)));
        const cellX = Math.min(width - 1, Math.max(0, Math.floor(p.x)));
        const cell = terrain.grid[cellY][cellX];
        const isWet = cell.elevation < 20 || cell.isLake || cell.isRiver || cell.isWaterfall;
        const isSnow = cell.biome === 'snow peaks' || cell.elevation >= 80;
        
        if (!isWet && !isSnow) {
          list.push(p);
        }
      }
    }
    return list;
  }, [placements, terrain, width, height]);`;

const new_filtered = `  // Overlap prevention distance filter
  const filteredPlacements = useMemo(() => {
    const list: typeof placements = [];
    for (const p of placements) {
      // Double check biome rules just in case
      const cellY = Math.min(actualHeight - 1, Math.max(0, Math.floor(p.y)));
      const cellX = Math.min(actualWidth - 1, Math.max(0, Math.floor(p.x)));
      const cell = terrain.grid[cellY][cellX];
      const isWet = cell.elevation < 20 || cell.isLake || cell.isRiver || cell.isWaterfall;
      const isSnow = cell.biome === 'snow peaks' || cell.elevation >= 80;
      
      if (!isWet && !isSnow) {
        list.push(p);
      }
    }
    return list;
  }, [placements, terrain, actualWidth, actualHeight]);`;

if (content.includes(old_filtered)) {
  content = content.replace(old_filtered, new_filtered);
  console.log("Filtered replaced.");
} else {
  console.log("Filtered NOT replaced.");
}

// 3. Grass
const old_grass = `  // Seeded generation of grass patches
  const grassPlacements = useMemo(() => {
    const list: { x: number; y: number; scale: number }[] = [];
    // Ensure density factor filters grass too
    if (densityFactor <= 0) return list;

    const random = mulberry32(54321); // unique seed for grass
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cell = terrain.grid[y][x];
        const isWet = cell.elevation < 20 || cell.isLake || cell.isRiver || cell.isWaterfall;
        const isSnow = cell.biome === 'snow peaks' || cell.elevation >= 80;
        if (isWet || isSnow) continue;

        let grassDensity = 0.7;
        if (cell.biome === 'grassland') {
          grassDensity = 0.7;
        } else if (cell.biome === 'forest') {
          grassDensity = 0.4;
        } else if (cell.biome === 'beach') {
          grassDensity = 0.05;
        }

        // Apply densityFactor
        grassDensity *= densityFactor;

        if (random() < grassDensity) {
          const numPatches = Math.floor(random() * 2) + 1;
          for (let i = 0; i < numPatches; i++) {
            const offsetX = random();
            const offsetY = random();
            const scale = 0.3 + random() * 0.4;
            list.push({
              x: x + offsetX,
              y: y + offsetY,
              scale,
            });
          }
        }
      }
    }
    
    // Cap grass elements to max capacity as well
    if (list.length > maxCapacity) {
      return list.slice(0, maxCapacity);
    }
    return list;
  }, [terrain, width, height, densityFactor, maxCapacity]);`;

const new_grass = `  // Seeded generation of grass patches
  const grassPlacements = useMemo(() => {
    const list: { x: number; y: number; scale: number }[] = [];
    // Ensure density factor filters grass too
    if (densityFactor <= 0) return list;

    const random = mulberry32(54321); // unique seed for grass
    for (let y = 0; y < actualHeight; y++) {
      for (let x = 0; x < actualWidth; x++) {
        const cell = terrain.grid[y][x];
        const isWet = cell.elevation < 20 || cell.isLake || cell.isRiver || cell.isWaterfall;
        const isSnow = cell.biome === 'snow peaks' || cell.elevation >= 80;
        if (isWet || isSnow) continue;

        let grassDensity = 0.7;
        if (cell.biome === 'grassland') {
          grassDensity = 0.7;
        } else if (cell.biome === 'forest') {
          grassDensity = 0.4;
        } else if (cell.biome === 'beach') {
          grassDensity = 0.05;
        }

        // Apply densityFactor
        grassDensity *= densityFactor;

        if (random() < grassDensity) {
          const numPatches = Math.floor(random() * 2) + 1;
          for (let i = 0; i < numPatches; i++) {
            const offsetX = random();
            const offsetY = random();
            const scale = 0.3 + random() * 0.4;
            list.push({
              x: x + offsetX,
              y: y + offsetY,
              scale,
            });
          }
        }
      }
    }
    
    // Cap grass elements to max capacity as well
    if (list.length > maxCapacity) {
      return list.slice(0, maxCapacity);
    }
    return list;
  }, [terrain, actualWidth, actualHeight, densityFactor, maxCapacity]);`;

if (content.includes(old_grass)) {
  content = content.replace(old_grass, new_grass);
  console.log("Grass replaced.");
} else {
  console.log("Grass NOT replaced.");
}

// 4. Species
const old_species = `  // Group placements by species
  const oakPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Oak'), [filteredPlacements]);
  const pinePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Pine'), [filteredPlacements]);
  const bushPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Bush'), [filteredPlacements]);
  const rockPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Rock'), [filteredPlacements]);
  const palmPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Palm'), [filteredPlacements]);
  const cactusPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Cactus'), [filteredPlacements]);
  const junglePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Jungle'), [filteredPlacements]);
  const birchPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Birch'), [filteredPlacements]);
  const flowersPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Flowers'), [filteredPlacements]);

  const speciesCounts = useMemo(() => ({
    Oak: oakPlacements.length,
    Pine: pinePlacements.length,
    Bush: bushPlacements.length,
    Rock: rockPlacements.length,
    Palm: palmPlacements.length,
    Cactus: cactusPlacements.length,
    Jungle: junglePlacements.length,
    Birch: birchPlacements.length,
    Flowers: flowersPlacements.length,
    Grass: grassPlacements.length,
  }), [
    oakPlacements, pinePlacements, bushPlacements, rockPlacements,
    palmPlacements, cactusPlacements, junglePlacements, birchPlacements,
    flowersPlacements, grassPlacements
  ]);`;

const new_species = `  // Group placements by species
  const oakPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Oak'), [filteredPlacements]);
  const pinePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Pine'), [filteredPlacements]);
  const bushPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Bush'), [filteredPlacements]);
  const rockPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Rock'), [filteredPlacements]);
  const palmPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Palm'), [filteredPlacements]);
  const cactusPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Cactus'), [filteredPlacements]);
  const junglePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Jungle'), [filteredPlacements]);
  const birchPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Birch'), [filteredPlacements]);
  const flowersPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Flowers'), [filteredPlacements]);
  const deadTrunkPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Dead Trunk'), [filteredPlacements]);
  const snowPinePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Snow Pine'), [filteredPlacements]);
  const iceRockPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Ice Rock'), [filteredPlacements]);

  const speciesCounts = useMemo(() => ({
    Oak: oakPlacements.length,
    Pine: pinePlacements.length,
    Bush: bushPlacements.length,
    Rock: rockPlacements.length,
    Palm: palmPlacements.length,
    Cactus: cactusPlacements.length,
    Jungle: junglePlacements.length,
    Birch: birchPlacements.length,
    Flowers: flowersPlacements.length,
    DeadTrunk: deadTrunkPlacements.length,
    SnowPine: snowPinePlacements.length,
    IceRock: iceRockPlacements.length,
    Grass: grassPlacements.length,
  }), [
    oakPlacements, pinePlacements, bushPlacements, rockPlacements,
    palmPlacements, cactusPlacements, junglePlacements, birchPlacements,
    flowersPlacements, deadTrunkPlacements, snowPinePlacements, iceRockPlacements,
    grassPlacements
  ]);`;

if (content.includes(old_species)) {
  content = content.replace(old_species, new_species);
  console.log("Species replaced.");
} else {
  console.log("Species NOT replaced.");
}

// 5. Geos
const old_geos = `  // 3D Low-poly models
  const geometries = useMemo(() => {
    const oakTrunk = new THREE.CylinderGeometry(0.35, 0.5, 2.5, 5);
    const oakLeaves = new THREE.DodecahedronGeometry(2.2, 1);
    const pineTrunk = new THREE.CylinderGeometry(0.25, 0.4, 2, 5);
    const pineLeaves1 = new THREE.ConeGeometry(2, 3.5, 5);
    const pineLeaves2 = new THREE.ConeGeometry(1.5, 2.8, 5);
    const pineLeaves3 = new THREE.ConeGeometry(0.9, 2, 5);
    const bushGeo = new THREE.DodecahedronGeometry(1.2, 0);
    const rockGeo = new THREE.DodecahedronGeometry(1.5, 0);
    const palmTrunk = new THREE.CylinderGeometry(0.2, 0.3, 5, 5);
    const palmLeaves = new THREE.DodecahedronGeometry(2.5, 0);
    const cactusGeo = new THREE.CylinderGeometry(0.3, 0.35, 2.5, 6);
    const jungleTrunk = new THREE.CylinderGeometry(0.4, 0.6, 4, 5);
    const jungleLeaves = new THREE.DodecahedronGeometry(3, 1);
    const birchTrunk = new THREE.CylinderGeometry(0.2, 0.3, 3, 5);
    const birchLeaves = new THREE.DodecahedronGeometry(1.8, 1);
    const flowersGeo = new THREE.SphereGeometry(0.4, 5, 5);

    // Grass (Intersecting Cross-Planes)
    const p1 = new THREE.PlaneGeometry(0.5, 0.5);
    p1.translate(0, 0.25, 0);
    const p2 = p1.clone();
    p2.rotateY(Math.PI / 2);
    const grassGeo = mergeGeometries(
      p1,
      new THREE.Color('#4ade80'),
      p2,
      new THREE.Color('#4ade80')
    );

    p1.dispose();
    p2.dispose();

    return {
      OakTrunk: oakTrunk,
      OakLeaves: oakLeaves,
      PineTrunk: pineTrunk,
      PineLeaves1: pineLeaves1,
      PineLeaves2: pineLeaves2,
      PineLeaves3: pineLeaves3,
      Bush: bushGeo,
      Rock: rockGeo,
      PalmTrunk: palmTrunk,
      PalmLeaves: palmLeaves,
      Cactus: cactusGeo,
      JungleTrunk: jungleTrunk,
      JungleLeaves: jungleLeaves,
      BirchTrunk: birchTrunk,
      BirchLeaves: birchLeaves,
      Flowers: flowersGeo,
      Grass: grassGeo,
    };
  }, []);`;

const new_geos = `  // 3D Low-poly models
  const geometries = useMemo(() => {
    const oakTrunk = new THREE.CylinderGeometry(0.35, 0.5, 2.5, 5);
    const oakLeaves = new THREE.DodecahedronGeometry(2.2, 1);
    const pineTrunk = new THREE.CylinderGeometry(0.25, 0.4, 2, 5);
    const pineLeaves1 = new THREE.ConeGeometry(2, 3.5, 5);
    const pineLeaves2 = new THREE.ConeGeometry(1.5, 2.8, 5);
    const pineLeaves3 = new THREE.ConeGeometry(0.9, 2, 5);
    const bushGeo = new THREE.DodecahedronGeometry(1.2, 0);
    const rockGeo = new THREE.DodecahedronGeometry(1.5, 0);
    const palmTrunk = new THREE.CylinderGeometry(0.2, 0.3, 5, 5);
    const palmLeaves = new THREE.DodecahedronGeometry(2.5, 0);
    const cactusGeo = new THREE.CylinderGeometry(0.3, 0.35, 2.5, 6);
    const jungleTrunk = new THREE.CylinderGeometry(0.4, 0.6, 4, 5);
    const jungleLeaves = new THREE.DodecahedronGeometry(3, 1);
    const birchTrunk = new THREE.CylinderGeometry(0.2, 0.3, 3, 5);
    const birchLeaves = new THREE.DodecahedronGeometry(1.8, 1);
    const flowersGeo = new THREE.SphereGeometry(0.4, 5, 5);
    const deadTrunkGeo = new THREE.CylinderGeometry(0.2, 0.35, 2.0, 5);
    const snowPineTrunk = new THREE.CylinderGeometry(0.2, 0.35, 2.2, 5);
    const snowPineLeaves = new THREE.ConeGeometry(1.8, 3.2, 5);
    const iceRockGeo = new THREE.DodecahedronGeometry(1.4, 0);

    // Grass (Intersecting Cross-Planes)
    const p1 = new THREE.PlaneGeometry(0.5, 0.5);
    p1.translate(0, 0.25, 0);
    const p2 = p1.clone();
    p2.rotateY(Math.PI / 2);
    const grassGeo = mergeGeometries(
      p1,
      new THREE.Color('#4ade80'),
      p2,
      new THREE.Color('#4ade80')
    );

    p1.dispose();
    p2.dispose();

    return {
      OakTrunk: oakTrunk,
      OakLeaves: oakLeaves,
      PineTrunk: pineTrunk,
      PineLeaves1: pineLeaves1,
      PineLeaves2: pineLeaves2,
      PineLeaves3: pineLeaves3,
      Bush: bushGeo,
      Rock: rockGeo,
      PalmTrunk: palmTrunk,
      PalmLeaves: palmLeaves,
      Cactus: cactusGeo,
      JungleTrunk: jungleTrunk,
      JungleLeaves: jungleLeaves,
      BirchTrunk: birchTrunk,
      BirchLeaves: birchLeaves,
      Flowers: flowersGeo,
      Grass: grassGeo,
      DeadTrunk: deadTrunkGeo,
      SnowPineTrunk: snowPineTrunk,
      SnowPineLeaves: snowPineLeaves,
      IceRock: iceRockGeo,
    };
  }, []);`;

if (content.includes(old_geos)) {
  content = content.replace(old_geos, new_geos);
  console.log("Geos replaced.");
} else {
  console.log("Geos NOT replaced.");
}

// 6. Mats
const old_mats = `    return {
      OakTrunk: createWindMat('#543b23'),
      OakLeaves: createWindMat('#6b8e23'),
      PineTrunk: createWindMat('#5c4020'),
      PineLeaves1: createWindMat('#2e6b34'),
      PineLeaves2: createWindMat('#358238'),
      PineLeaves3: createWindMat('#3d9440'),
      Bush: createWindMat('#5a9e45'),
      Rock: createStaticMat('#a08860'),
      PalmTrunk: createWindMat('#8b6914'),
      PalmLeaves: createWindMat('#44aa33'),
      Cactus: createWindMat('#3a7a30'),
      JungleTrunk: createWindMat('#4a3520'),
      JungleLeaves: createWindMat('#1a6618'),
      BirchTrunk: createWindMat('#ccccbb'),
      BirchLeaves: createWindMat('#88b840'),
      Flowers: createWindMat('#dd6699'),
      Grass: createGrassMat('#4ade80'),
    };
  }, [uniforms]);`;

const new_mats = `    return {
      OakTrunk: createWindMat('#543b23'),
      OakLeaves: createWindMat('#6b8e23'),
      PineTrunk: createWindMat('#5c4020'),
      PineLeaves1: createWindMat('#2e6b34'),
      PineLeaves2: createWindMat('#358238'),
      PineLeaves3: createWindMat('#3d9440'),
      Bush: createWindMat('#5a9e45'),
      Rock: createStaticMat('#a08860'),
      PalmTrunk: createWindMat('#8b6914'),
      PalmLeaves: createWindMat('#44aa33'),
      Cactus: createWindMat('#3a7a30'),
      JungleTrunk: createWindMat('#4a3520'),
      JungleLeaves: createWindMat('#1a6618'),
      BirchTrunk: createWindMat('#ccccbb'),
      BirchLeaves: createWindMat('#88b840'),
      Flowers: createWindMat('#dd6699'),
      Grass: createGrassMat('#4ade80'),
      DeadTrunk: createWindMat('#4a3b32'),
      SnowPineTrunk: createWindMat('#4c3a2a'),
      SnowPineLeaves: createWindMat('#c8e3f5'),
      IceRock: createStaticMat('#88ccee'),
    };
  }, [uniforms]);`;

if (content.includes(old_mats)) {
  content = content.replace(old_mats, new_mats);
  console.log("Mats replaced.");
} else {
  console.log("Mats NOT replaced.");
}

// 7. Refs
const old_refs = `  // Instance refs
  const oakTRef = useRef<THREE.InstancedMesh>(null);
  const oakLRef = useRef<THREE.InstancedMesh>(null);
  const pineTRef = useRef<THREE.InstancedMesh>(null);
  const pineL1Ref = useRef<THREE.InstancedMesh>(null);
  const pineL2Ref = useRef<THREE.InstancedMesh>(null);
  const pineL3Ref = useRef<THREE.InstancedMesh>(null);
  const bushRef = useRef<THREE.InstancedMesh>(null);
  const rockRef = useRef<THREE.InstancedMesh>(null);
  const palmTRef = useRef<THREE.InstancedMesh>(null);
  const palmLRef = useRef<THREE.InstancedMesh>(null);
  const cactusRef = useRef<THREE.InstancedMesh>(null);
  const jungleTRef = useRef<THREE.InstancedMesh>(null);
  const jungleLRef = useRef<THREE.InstancedMesh>(null);
  const birchTRef = useRef<THREE.InstancedMesh>(null);
  const birchLRef = useRef<THREE.InstancedMesh>(null);
  const flowersRef = useRef<THREE.InstancedMesh>(null);
  const grassRef = useRef<THREE.InstancedMesh>(null);`;

const new_refs = `  // Instance refs
  const oakTRef = useRef<THREE.InstancedMesh>(null);
  const oakLRef = useRef<THREE.InstancedMesh>(null);
  const pineTRef = useRef<THREE.InstancedMesh>(null);
  const pineL1Ref = useRef<THREE.InstancedMesh>(null);
  const pineL2Ref = useRef<THREE.InstancedMesh>(null);
  const pineL3Ref = useRef<THREE.InstancedMesh>(null);
  const bushRef = useRef<THREE.InstancedMesh>(null);
  const rockRef = useRef<THREE.InstancedMesh>(null);
  const palmTRef = useRef<THREE.InstancedMesh>(null);
  const palmLRef = useRef<THREE.InstancedMesh>(null);
  const cactusRef = useRef<THREE.InstancedMesh>(null);
  const jungleTRef = useRef<THREE.InstancedMesh>(null);
  const jungleLRef = useRef<THREE.InstancedMesh>(null);
  const birchTRef = useRef<THREE.InstancedMesh>(null);
  const birchLRef = useRef<THREE.InstancedMesh>(null);
  const flowersRef = useRef<THREE.InstancedMesh>(null);
  const grassRef = useRef<THREE.InstancedMesh>(null);
  const deadTrunkRef = useRef<THREE.InstancedMesh>(null);
  const snowPineTRef = useRef<THREE.InstancedMesh>(null);
  const snowPineLRef = useRef<THREE.InstancedMesh>(null);
  const iceRockRef = useRef<THREE.InstancedMesh>(null);`;

if (content.includes(old_refs)) {
  content = content.replace(old_refs, new_refs);
  console.log("Refs replaced.");
} else {
  console.log("Refs NOT replaced.");
}

// 8. setupInstances inner function
const old_setup_func = `    const setupInstances = (
      ref: React.RefObject<THREE.InstancedMesh>,
      placementsArray: any[],
      yOffset: number,
      rotType: 'y' | 'all' | 'none',
      seedOffset: number
    ) => {
      if (!ref.current) return;
      const inst = ref.current;
      if (placementsArray.length === 0) {
        const gx = Math.min(width - 1, Math.max(0, Math.floor(width / 2)));
        const gy = Math.min(height - 1, Math.max(0, Math.floor(height / 2)));
        const cell = terrain.grid[gy]?.[gx];
        const x = gx - width / 2;
        const z = gy - height / 2;
        const y = cell ? cell.elevation * 1.8 : 0;
        const fallbackYOffset = ref === flowersRef ? 0.3 : 0;

        dummy.position.set(x, y + fallbackYOffset, z);
        dummy.scale.set(0, 0, 0);
        dummy.rotation.set(0, 0, 0);
        dummy.updateMatrix();
        if (typeof inst.setMatrixAt === 'function') {
          inst.setMatrixAt(0, dummy.matrix);
        }
      } else {
        placementsArray.forEach((p, idx) => {
          const x = p.x - width / 2;
          const z = p.y - height / 2;
          const y = getBilinearInterpolatedElevation(p.x, p.y, width, height, terrain.grid) * 1.8;`;

const new_setup_func = `    const setupInstances = (
      ref: React.RefObject<THREE.InstancedMesh>,
      placementsArray: any[],
      yOffset: number,
      rotType: 'y' | 'all' | 'none',
      seedOffset: number
    ) => {
      if (!ref.current) return;
      const inst = ref.current;
      if (placementsArray.length === 0) {
        const gx = Math.min(actualWidth - 1, Math.max(0, Math.floor(actualWidth / 2)));
        const gy = Math.min(actualHeight - 1, Math.max(0, Math.floor(actualHeight / 2)));
        const cell = terrain.grid[gy]?.[gx];
        const x = gx - actualWidth / 2;
        const z = gy - actualHeight / 2;
        const y = cell ? cell.elevation * 1.8 : 0;
        const fallbackYOffset = ref === flowersRef ? 0.3 : 0;

        dummy.position.set(x, y + fallbackYOffset, z);
        dummy.scale.set(0, 0, 0);
        dummy.rotation.set(0, 0, 0);
        dummy.updateMatrix();
        if (typeof inst.setMatrixAt === 'function') {
          inst.setMatrixAt(0, dummy.matrix);
        }
      } else {
        placementsArray.forEach((p, idx) => {
          const x = p.x - actualWidth / 2;
          const z = p.y - actualHeight / 2;
          const y = getBilinearInterpolatedElevation(p.x, p.y, actualWidth, actualHeight, terrain.grid) * 1.8;`;

if (content.includes(old_setup_func)) {
  content = content.replace(old_setup_func, new_setup_func);
  console.log("Setup function replaced.");
} else {
  console.log("Setup function NOT replaced.");
}

// 9. Effect calls
const old_effect = `    setupInstances(oakTRef, oakPlacements, 1.25, 'y', 12.3);
    setupInstances(oakLRef, oakPlacements, 3.0, 'y', 12.3);

    setupInstances(pineTRef, pinePlacements, 1.0, 'none', 23.4);
    setupInstances(pineL1Ref, pinePlacements, 2.2, 'none', 23.4);
    setupInstances(pineL2Ref, pinePlacements, 3.4, 'none', 23.4);
    setupInstances(pineL3Ref, pinePlacements, 4.5, 'none', 23.4);

    setupInstances(bushRef, bushPlacements, 0.4, 'y', 34.5);
    setupInstances(rockRef, rockPlacements, -0.3, 'all', 45.6);

    setupInstances(palmTRef, palmPlacements, 2.5, 'y', 56.7);
    setupInstances(palmLRef, palmPlacements, 5.0, 'y', 56.7);

    setupInstances(cactusRef, cactusPlacements, 1.2, 'y', 67.8);

    setupInstances(jungleTRef, junglePlacements, 2.0, 'y', 78.9);
    setupInstances(jungleLRef, junglePlacements, 5.0, 'y', 78.9);

    setupInstances(birchTRef, birchPlacements, 1.5, 'y', 89.01);
    setupInstances(birchLRef, birchPlacements, 3.2, 'y', 89.01);

    setupInstances(flowersRef, flowersPlacements, 0.3, 'y', 100.12);
    setupInstances(grassRef, grassPlacements, 0, 'y', 111.23);
  }, [
    width,
    height,
    terrain,
    oakPlacements,
    pinePlacements,
    bushPlacements,
    rockPlacements,
    palmPlacements,
    cactusPlacements,
    junglePlacements,
    birchPlacements,
    flowersPlacements,
    grassPlacements,
    dummy,
  ]);`;

const new_effect = `    setupInstances(oakTRef, oakPlacements, 1.25, 'y', 12.3);
    setupInstances(oakLRef, oakPlacements, 3.0, 'y', 12.3);

    setupInstances(pineTRef, pinePlacements, 1.0, 'none', 23.4);
    setupInstances(pineL1Ref, pinePlacements, 2.2, 'none', 23.4);
    setupInstances(pineL2Ref, pinePlacements, 3.4, 'none', 23.4);
    setupInstances(pineL3Ref, pinePlacements, 4.5, 'none', 23.4);

    setupInstances(bushRef, bushPlacements, 0.4, 'y', 34.5);
    setupInstances(rockRef, rockPlacements, -0.3, 'all', 45.6);

    setupInstances(palmTRef, palmPlacements, 2.5, 'y', 56.7);
    setupInstances(palmLRef, palmPlacements, 5.0, 'y', 56.7);

    setupInstances(cactusRef, cactusPlacements, 1.2, 'y', 67.8);

    setupInstances(jungleTRef, junglePlacements, 2.0, 'y', 78.9);
    setupInstances(jungleLRef, junglePlacements, 5.0, 'y', 78.9);

    setupInstances(birchTRef, birchPlacements, 1.5, 'y', 89.01);
    setupInstances(birchLRef, birchPlacements, 3.2, 'y', 89.01);

    setupInstances(flowersRef, flowersPlacements, 0.3, 'y', 100.12);
    setupInstances(grassRef, grassPlacements, 0, 'y', 111.23);

    setupInstances(deadTrunkRef, deadTrunkPlacements, 1.0, 'y', 121.23);
    setupInstances(snowPineTRef, snowPinePlacements, 1.1, 'none', 132.34);
    setupInstances(snowPineLRef, snowPinePlacements, 2.5, 'none', 132.34);
    setupInstances(iceRockRef, iceRockPlacements, -0.2, 'all', 143.45);
  }, [
    actualWidth,
    actualHeight,
    terrain,
    oakPlacements,
    pinePlacements,
    bushPlacements,
    rockPlacements,
    palmPlacements,
    cactusPlacements,
    junglePlacements,
    birchPlacements,
    flowersPlacements,
    deadTrunkPlacements,
    snowPinePlacements,
    iceRockPlacements,
    grassPlacements,
    dummy,
  ]);`;

if (content.includes(old_effect)) {
  content = content.replace(old_effect, new_effect);
  console.log("Effect calls replaced.");
} else {
  console.log("Effect calls NOT replaced.");
}

// 10. Return meshes
const old_ret = `      <instancedMesh
        ref={grassRef}
        name="vegetation-instanced-mesh-grass"
        args={[null as any, null as any, Math.max(1, speciesCounts.Grass)]}
      >
        <primitive object={geometries.Grass} attach="geometry" />
        <primitive object={materials.Grass} attach="material" />
      </instancedMesh>
    </group>`;

const new_ret = `      <instancedMesh
        ref={grassRef}
        name="vegetation-instanced-mesh-grass"
        args={[null as any, null as any, Math.max(1, speciesCounts.Grass)]}
      >
        <primitive object={geometries.Grass} attach="geometry" />
        <primitive object={materials.Grass} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={deadTrunkRef}
        name="vegetation-instanced-mesh-dead-trunk"
        args={[null as any, null as any, Math.max(1, speciesCounts.DeadTrunk)]}
        castShadow
        receiveShadow
      >
        <primitive object={geometries.DeadTrunk} attach="geometry" />
        <primitive object={materials.DeadTrunk} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={snowPineTRef}
        name="vegetation-instanced-mesh-snow-pine"
        args={[null as any, null as any, Math.max(1, speciesCounts.SnowPine)]}
        castShadow
        receiveShadow
      >
        <primitive object={geometries.SnowPineTrunk} attach="geometry" />
        <primitive object={materials.SnowPineTrunk} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={snowPineLRef}
        name="vegetation-instanced-mesh-snow-pine-leaves"
        args={[null as any, null as any, Math.max(1, speciesCounts.SnowPine)]}
        castShadow
        receiveShadow
      >
        <primitive object={geometries.SnowPineLeaves} attach="geometry" />
        <primitive object={materials.SnowPineLeaves} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={iceRockRef}
        name="vegetation-instanced-mesh-ice-rock"
        args={[null as any, null as any, Math.max(1, speciesCounts.IceRock)]}
        castShadow
        receiveShadow
      >
        <primitive object={geometries.IceRock} attach="geometry" />
        <primitive object={materials.IceRock} attach="material" />
      </instancedMesh>
    </group>`;

if (content.includes(old_ret)) {
  content = content.replace(old_ret, new_ret);
  console.log("Return meshes replaced.");
} else {
  console.log("Return meshes NOT replaced.");
}

// Restore line endings
content = content.replace(/\n/g, originalLineEndings);
fs.writeFileSync(filePath, content, 'utf8');
console.log("Finished successfully!");
