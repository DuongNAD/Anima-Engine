import { describe, it, expect } from "vitest";
import { 
  generateOrganism, 
  generatePheromones, 
  generateFlora, 
  generateEnvironment 
} from "../../playground/frontend/MockDataGenerator";

describe("MockDataGenerator", () => {
  it("generates correct number of segments and parent-child hierarchy", () => {
    const segments = generateOrganism(1, "prey", 5, 0, 0, 0, 0);
    expect(segments.length).toBe(5);
    
    // First segment should have parent_segment_id as null
    expect(segments[0].parent_segment_id).toBeNull();
    
    // Subsequent segments should point to their parent segment
    for (let i = 1; i < segments.length; i++) {
      expect(segments[i].parent_segment_id).toBe(i - 1);
    }
  });

  it("generates pheromone grid with correct dimensions", () => {
    const grid = generatePheromones(10, 10, Date.now());
    expect(grid.width).toBe(10);
    expect(grid.height).toBe(10);
    expect(grid.grid.length).toBe(100);
  });

  it("generates flora with expected count and fields", () => {
    const flora = generateFlora();
    expect(flora.length).toBeGreaterThan(0);
    expect(flora[0]).toHaveProperty("x");
    expect(flora[0]).toHaveProperty("y");
    expect(flora[0]).toHaveProperty("energy");
  });

  it("generates environment state", () => {
    const env = generateEnvironment(Date.now());
    expect(env.clouds.length).toBeGreaterThan(0);
    expect(env.ambient_light).toBeGreaterThanOrEqual(0.6);
    expect(env.ambient_light).toBeLessThanOrEqual(1.0);
  });
});
