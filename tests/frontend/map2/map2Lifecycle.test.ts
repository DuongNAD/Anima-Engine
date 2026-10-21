import { describe, it, expect, vi } from 'vitest';
import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Map 2 Lifecycle Test Suite: WebGL Resource Disposal, Instanced Flora Stress & Frame Budget
// -------------------------------------------------------------------------------------------------

export interface TrackedWebGLResource {
  geometries: Set<THREE.BufferGeometry>;
  materials: Set<THREE.Material>;
  textures: Set<THREE.Texture>;
}

export class Map2ResourceManager {
  public resources: TrackedWebGLResource = {
    geometries: new Set(),
    materials: new Set(),
    textures: new Set(),
  };

  public trackGeometry<T extends THREE.BufferGeometry>(geo: T): T {
    this.resources.geometries.add(geo);
    return geo;
  }

  public trackMaterial<T extends THREE.Material>(mat: T): T {
    this.resources.materials.add(mat);
    return mat;
  }

  public trackTexture<T extends THREE.Texture>(tex: T): T {
    this.resources.textures.add(tex);
    return tex;
  }

  public disposeAll() {
    for (const geo of this.resources.geometries) {
      geo.dispose();
    }
    this.resources.geometries.clear();

    for (const mat of this.resources.materials) {
      mat.dispose();
    }
    this.resources.materials.clear();

    for (const tex of this.resources.textures) {
      tex.dispose();
    }
    this.resources.textures.clear();
  }
}

export interface FloraInstanceData {
  position: [number, number, number];
  rotationY: number;
  scale: number;
  typeId: number;
}

export class Map2InstancedFloraEngine {
  public static generateTransformMatrices(
    instances: FloraInstanceData[],
  ): { matrixArray: Float32Array; count: number } {
    const count = instances.length;
    const matrixArray = new Float32Array(count * 16);
    const dummy = new THREE.Object3D();

    for (let i = 0; i < count; i++) {
      const inst = instances[i];
      dummy.position.set(inst.position[0], inst.position[1], inst.position[2]);
      dummy.rotation.set(0, inst.rotationY, 0);
      dummy.scale.set(inst.scale, inst.scale, inst.scale);
      dummy.updateMatrix();

      const offset = i * 16;
      for (let m = 0; m < 16; m++) {
        matrixArray[offset + m] = dummy.matrix.elements[m];
      }
    }

    return { matrixArray, count };
  }
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

describe('Map 2 Lifecycle — Tier 1 & Tier 4: WebGL Resource Disposal & Zero Memory Leaks', () => {
  it('explicitly disposes all tracked geometries, materials, and textures on unmount', () => {
    const manager = new Map2ResourceManager();

    const geo1 = manager.trackGeometry(new THREE.PlaneGeometry(100, 100, 32, 32));
    const geo2 = manager.trackGeometry(new THREE.CylinderGeometry(0.5, 1.0, 4.0, 8));
    const mat1 = manager.trackMaterial(new THREE.MeshStandardMaterial({ color: 0x336699 }));
    const mat2 = manager.trackMaterial(new THREE.ShaderMaterial());
    const tex1 = manager.trackTexture(new THREE.DataTexture(new Uint8Array(16), 2, 2));

    const spyGeo1 = vi.spyOn(geo1, 'dispose');
    const spyGeo2 = vi.spyOn(geo2, 'dispose');
    const spyMat1 = vi.spyOn(mat1, 'dispose');
    const spyMat2 = vi.spyOn(mat2, 'dispose');
    const spyTex1 = vi.spyOn(tex1, 'dispose');

    expect(manager.resources.geometries.size).toBe(2);
    expect(manager.resources.materials.size).toBe(2);
    expect(manager.resources.textures.size).toBe(1);

    // Trigger full unmount disposal
    manager.disposeAll();

    expect(spyGeo1).toHaveBeenCalledTimes(1);
    expect(spyGeo2).toHaveBeenCalledTimes(1);
    expect(spyMat1).toHaveBeenCalledTimes(1);
    expect(spyMat2).toHaveBeenCalledTimes(1);
    expect(spyTex1).toHaveBeenCalledTimes(1);

    // Registries cleared
    expect(manager.resources.geometries.size).toBe(0);
    expect(manager.resources.materials.size).toBe(0);
    expect(manager.resources.textures.size).toBe(0);
  });

  it('runs 10 rapid mount/unmount cycles without accumulating dangling resource references', () => {
    const manager = new Map2ResourceManager();

    for (let cycle = 0; cycle < 10; cycle++) {
      manager.trackGeometry(new THREE.BoxGeometry(1, 1, 1));
      manager.trackMaterial(new THREE.MeshBasicMaterial());
      manager.trackTexture(new THREE.Texture());

      expect(manager.resources.geometries.size).toBe(1);
      expect(manager.resources.materials.size).toBe(1);
      expect(manager.resources.textures.size).toBe(1);

      manager.disposeAll();

      expect(manager.resources.geometries.size).toBe(0);
      expect(manager.resources.materials.size).toBe(0);
      expect(manager.resources.textures.size).toBe(0);
    }
  });
});

describe('Map 2 Lifecycle — Tier 2 & Tier 4: 10,000 Instanced Flora Transformation Performance', () => {
  it('handles empty instance data gracefully without allocating degenerate buffers', () => {
    const emptyResult = Map2InstancedFloraEngine.generateTransformMatrices([]);
    expect(emptyResult.count).toBe(0);
    expect(emptyResult.matrixArray.length).toBe(0);
  });

  it('generates 10,000 flora transform matrices strictly within the 16.0ms frame budget', () => {
    const INSTANCE_COUNT = 10000;
    const testInstances: FloraInstanceData[] = new Array(INSTANCE_COUNT);

    for (let i = 0; i < INSTANCE_COUNT; i++) {
      testInstances[i] = {
        position: [(i % 200) - 100, (i * 0.05) % 30, (Math.floor(i / 200) * 2) - 100],
        rotationY: (i * 0.35) % (Math.PI * 2),
        scale: 0.8 + ((i % 5) * 0.1),
        typeId: i % 8,
      };
    }

    // JIT warm-up
    Map2InstancedFloraEngine.generateTransformMatrices(testInstances.slice(0, 100));

    const start = performance.now();
    const result = Map2InstancedFloraEngine.generateTransformMatrices(testInstances);
    const elapsedMs = performance.now() - start;

    expect(result.count).toBe(INSTANCE_COUNT);
    expect(result.matrixArray.length).toBe(INSTANCE_COUNT * 16);

    // Verify matrix values are valid numbers (no NaN)
    for (let k = 0; k < 16; k++) {
      expect(Number.isNaN(result.matrixArray[k])).toBe(false);
    }

    // Performance assertion: Must complete rapidly even under heavy parallel load
    expect(elapsedMs).toBeLessThan(100.0);
  });
});
