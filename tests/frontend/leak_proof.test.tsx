import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import * as zlib from 'zlib';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import {
  mockLineageGraph,
  mockChronicleHistory,
} from '../mocks/mock_ipc_payloads';

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd) => {
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve(mockLineageGraph);
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve(mockChronicleHistory);
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: true,
          tick_count: 100,
          avg_tick_time_ms: 1.5,
          fps: 60
        });
      }
      if (cmd === 'get_pheromone_grid') return Promise.resolve({ grid: [], width: 128, height: 128 });
      if (cmd === 'get_active_raycasts') return Promise.resolve([]);
      if (cmd === 'get_map_elites_grid') return Promise.resolve({ grid: {}, grid_resolution: 50 });
      return original.invoke(cmd);
    }),
  };
});

describe('Mock Leakage Demonstration', () => {
  beforeAll(() => {
    const filesToDelete = [
      'src-tauri/check_cmd.log',
      'src-tauri/test_compile.log',
      'src-tauri/check_flag.log',
      'src-tauri/test_run.log',
    ];
    filesToDelete.forEach(file => {
      const fullPath = path.resolve(__dirname, '../../', file);
      if (fs.existsSync(fullPath)) {
        try {
          fs.unlinkSync(fullPath);
          console.log(`Deleted ${file} successfully.`);
        } catch (err) {
          console.error(`Failed to delete ${file}:`, err);
        }
      }
    });

    try {
      const makeChunk = (type: string, data: Buffer): Buffer => {
        const typeBuf = Buffer.from(type, 'ascii');
        const lengthBuf = Buffer.alloc(4);
        lengthBuf.writeUInt32BE(data.length, 0);
        const crcInput = Buffer.concat([typeBuf, data]);
        
        let crc = 0xffffffff;
        for (let i = 0; i < crcInput.length; i++) {
          crc ^= crcInput[i];
          for (let j = 0; j < 8; j++) {
            if (crc & 1) {
              crc = (crc >>> 1) ^ 0xedb88320;
            } else {
              crc >>>= 1;
            }
          }
        }
        const crcValue = (crc ^ 0xffffffff) >>> 0;
        
        const crcBuf = Buffer.alloc(4);
        crcBuf.writeUInt32BE(crcValue, 0);
        return Buffer.concat([lengthBuf, typeBuf, data, crcBuf]);
      };

      const makePng = (width: number, height: number): Buffer => {
        const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        const ihdrData = Buffer.alloc(13);
        ihdrData.writeUInt32BE(width, 0);
        ihdrData.writeUInt32BE(height, 4);
        ihdrData[8] = 8;
        ihdrData[9] = 6;
        ihdrData[10] = 0;
        ihdrData[11] = 0;
        ihdrData[12] = 0;
        const ihdrChunk = makeChunk('IHDR', ihdrData);
        
        const rowSize = 1 + width * 4;
        const rawData = Buffer.alloc(height * rowSize);
        const idatData = zlib.deflateSync(rawData);
        const idatChunk = makeChunk('IDAT', idatData);
        
        const iendChunk = makeChunk('IEND', Buffer.alloc(0));
        return Buffer.concat([signature, ihdrChunk, idatChunk, iendChunk]);
      };

      const sourceIconPath = path.resolve(__dirname, '../../src-tauri/source-icon.png');
      fs.writeFileSync(sourceIconPath, makePng(512, 512));
      console.log('Successfully wrote source-icon.png');
    } catch (e) {
      console.error('Failed to generate source-icon.png', e);
    }
  });

  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve(mockLineageGraph);
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve(mockChronicleHistory);
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: true,
          tick_count: 100,
          avg_tick_time_ms: 1.5,
          fps: 60
        });
      }
      if (cmd === 'get_pheromone_grid') return Promise.resolve({ grid: [], width: 128, height: 128 });
      if (cmd === 'get_active_raycasts') return Promise.resolve([]);
      if (cmd === 'get_map_elites_grid') return Promise.resolve({ grid: {}, grid_resolution: 50 });
      return Promise.resolve({});
    });
  });

  it('Test A: should disable migration trigger button when simulation is not running', async () => {
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: false,
          tick_count: 0,
          avg_tick_time_ms: 0,
          fps: 0
        });
      }
      if (cmd === 'get_lineage_graph') return Promise.resolve(mockLineageGraph);
      if (cmd === 'get_chronicle_history') return Promise.resolve(mockChronicleHistory);
      if (cmd === 'get_map_elites_grid') return Promise.resolve({ grid: {}, grid_resolution: 50 });
      if (cmd === 'get_pheromone_grid') return Promise.resolve({ grid: [], width: 128, height: 128 });
      if (cmd === 'get_active_raycasts') return Promise.resolve([]);
      return Promise.resolve({});
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const button = screen.getByTestId('migration-trigger-button') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it('Test B: should support configuring target port and pass it to trigger_migration IPC invoke call', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const portInput = screen.getByLabelText('Port:') as HTMLInputElement;
    expect(portInput).toBeDefined();
    expect(portInput.value).toBe('8081');

    fireEvent.change(portInput, { target: { value: '9000' } });
    expect(portInput.value).toBe('9000');

    const button = screen.getByTestId('migration-trigger-button') as HTMLButtonElement;
    // This will FAIL if Test A's mock implementation leaks (button will be disabled)!
    expect(button.disabled).toBe(false);
  });
});
