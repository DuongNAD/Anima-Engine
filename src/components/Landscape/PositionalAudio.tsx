import React, { useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import { audioManager } from './utils/audioManager';

interface PositionalAudioProps {
  id: string;
  position: [number, number, number];
  volume?: number;
  isMuted?: boolean;
}

export const PositionalAudio: React.FC<PositionalAudioProps> = ({
  id,
  position,
  volume = 1.0,
  isMuted = false,
}) => {
  useEffect(() => {
    audioManager.createSpatialSource(id);
  }, [id]);

  useFrame(() => {
    audioManager.updateSpatialSource(id, position[0], position[1], position[2]);
  });

  return (
    <group name="audio-group" userData={{ audioId: id, position, volume, isMuted }}>
      <mesh name="audio-source-mesh" position={position}>
        <sphereGeometry args={[0.2, 8, 8]} />
        <meshBasicMaterial color="#ef4444" wireframe />
      </mesh>
    </group>
  );
};

export default PositionalAudio;
