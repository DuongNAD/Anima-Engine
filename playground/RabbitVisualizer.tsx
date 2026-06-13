import { useRef, useEffect, useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { invoke } from '@tauri-apps/api/core';
const invokeTauri = invoke;

const TOTAL_PARTS = 10;

// Pre-defined low-poly geometries (4 to 8 segments) to ensure blocky silhouette
const geomBody = new THREE.SphereGeometry(1, 8, 6);
const geomHead = new THREE.SphereGeometry(1, 8, 6);
const geomSnout = new THREE.SphereGeometry(1, 6, 6);
const geomBlush = new THREE.SphereGeometry(1, 6, 4);
const geomEye = new THREE.SphereGeometry(0.09, 6, 6);
const geomEyeGlint = new THREE.SphereGeometry(1, 4, 4);
const geomNose = new THREE.SphereGeometry(0.065, 6, 4);
const geomEar = new THREE.SphereGeometry(1, 6, 4);
const geomLeg = new THREE.SphereGeometry(1, 6, 6);
const geomTail = new THREE.SphereGeometry(1, 6, 6);
const geomMouth = new THREE.SphereGeometry(1, 6, 6);

// Pre-defined edges geometries for overlays
const edgesBody = new THREE.EdgesGeometry(geomBody);
const edgesHead = new THREE.EdgesGeometry(geomHead);
const edgesSnout = new THREE.EdgesGeometry(geomSnout);
const edgesBlush = new THREE.EdgesGeometry(geomBlush);
const edgesEye = new THREE.EdgesGeometry(geomEye);
const edgesEyeGlint = new THREE.EdgesGeometry(geomEyeGlint);
const edgesNose = new THREE.EdgesGeometry(geomNose);
const edgesEar = new THREE.EdgesGeometry(geomEar);
const edgesLeg = new THREE.EdgesGeometry(geomLeg);
const edgesTail = new THREE.EdgesGeometry(geomTail);
const edgesMouth = new THREE.EdgesGeometry(geomMouth);

// Helper to generate procedural rabbit morphology data in JS
function generateSingleRabbitJS(
    rootX: number,
    rootY: number,
    rootRotation: number,
    time: number,
    speedMultiplier: number,
    isEating: boolean
): Float32Array {
    const data = new Float32Array(TOTAL_PARTS * 13);
    
    // Add subtle hopping and breathing animation using time
    const t = time * speedMultiplier;
    const breathing = Math.sin(t * 4) * 0.04;
    const hopHeight = Math.max(0, Math.sin(t * 2)) * 0.6;
    const hopRotation = Math.sin(t * 2) * 0.08;
    
    // Smooth horizontal pacing
    const curX = rootX + Math.sin(t * 0.5) * 2.0;
    const curY = rootY + hopHeight - 0.5;
    const curRot = rootRotation + hopRotation;

    const cosR = Math.cos(curRot);
    const sinR = Math.sin(curRot);

    const localToWorld = (lx: number, ly: number, lz: number) => {
        return [
            curX + lx * cosR - ly * sinR,
            curY + lx * sinR + ly * cosR,
            lz
        ];
    };

    const writePart = (
        idx: number,
        x: number, y: number, z: number,
        rotX: number, rotY: number, rotZ: number,
        scaleX: number, scaleY: number, scaleZ: number,
        r: number, g: number, b: number,
        type: number
    ) => {
        const offset = idx * 13;
        data[offset] = x;
        data[offset + 1] = y;
        data[offset + 2] = z;
        data[offset + 3] = rotX;
        data[offset + 4] = rotY;
        data[offset + 5] = rotZ;
        data[offset + 6] = scaleX;
        data[offset + 7] = scaleY;
        data[offset + 8] = scaleZ;
        data[offset + 9] = r;
        data[offset + 10] = g;
        data[offset + 11] = b;
        data[offset + 12] = type;
    };

    // 0. Body (type 0.0)
    writePart(0, curX, curY, 0.0, 0, 0, curRot, (2.0 + breathing) * 1.6, (2.0 + breathing) * 1.0, (2.0 + breathing) * 1.0, 0.9, 0.9, 0.9, 0.0);

    // 1. Head (type 1.0)
    const [headX, headY, headZ] = localToWorld(1.8, 0.0, 0.0);
    writePart(1, headX, headY, headZ, 0, 0, curRot, (1.2 + breathing * 0.5) * 1.1, (1.2 + breathing * 0.5) * 0.9, (1.2 + breathing * 0.5) * 0.95, 0.95, 0.95, 0.95, 1.0);

    // 2. Left Ear (type 2.0)
    const earBreathing = Math.sin(t * 6) * 0.12;
    const [earLX, earLY, earLZ] = localToWorld(2.0, 0.8, 0.5);
    writePart(2, earLX, earLY, earLZ, 0, 0, curRot + 0.3 + earBreathing, 0.8 * 2.8, 0.8 * 0.35, 0.8 * 0.2, 0.85, 0.75, 0.75, 2.0);

    // 3. Right Ear (type 3.0)
    const [earRX, earRY, earRZ] = localToWorld(2.0, -0.8, -0.5);
    writePart(3, earRX, earRY, earRZ, 0, 0, curRot - 0.3 - earBreathing, 0.8 * 2.8, 0.8 * 0.35, 0.8 * 0.2, 0.85, 0.75, 0.75, 3.0);

    // 4. Front-Left Leg (type 4.0)
    const [flLegX, flLegY, flLegZ] = localToWorld(0.8 + Math.sin(t * 4 + Math.PI) * 0.15, -0.8 - hopHeight * 0.35, 0.5);
    writePart(4, flLegX, flLegY, flLegZ, 0, 0, curRot + Math.sin(t * 4 + Math.PI) * 0.25 - hopHeight * 0.3, 0.8 * 1.0, 0.8 * 1.3, 0.8 * 1.0, 0.82, 0.82, 0.82, 4.0);

    // 5. Front-Right Leg (type 5.0)
    const [frLegX, frLegY, frLegZ] = localToWorld(0.8 + Math.sin(t * 4) * 0.15, -0.8 - hopHeight * 0.35, -0.5);
    writePart(5, frLegX, frLegY, frLegZ, 0, 0, curRot + Math.sin(t * 4) * 0.25 - hopHeight * 0.3, 0.8 * 1.0, 0.8 * 1.3, 0.8 * 1.0, 0.82, 0.82, 0.82, 5.0);

    // 6. Hind-Left Leg (type 6.0)
    const [hlLegX, hlLegY, hlLegZ] = localToWorld(-1.2 - hopHeight * 0.1 + Math.sin(t * 4) * 0.1, -0.6 - hopHeight * 0.4, 0.6);
    writePart(6, hlLegX, hlLegY, hlLegZ, 0, 0, curRot + Math.sin(t * 4) * 0.15 - hopHeight * 0.3, 1.4 * 1.0, 1.4 * 1.3, 1.4 * 1.0, 0.8, 0.8, 0.8, 6.0);

    // 7. Hind-Right Leg (type 7.0)
    const [hrLegX, hrLegY, hrLegZ] = localToWorld(-1.2 - hopHeight * 0.1 + Math.sin(t * 4 + Math.PI) * 0.1, -0.6 - hopHeight * 0.4, -0.6);
    writePart(7, hrLegX, hrLegY, hrLegZ, 0, 0, curRot + Math.sin(t * 4 + Math.PI) * 0.15 - hopHeight * 0.3, 1.4 * 1.0, 1.4 * 1.3, 1.4 * 1.0, 0.8, 0.8, 0.8, 7.0);

    // 8. Tail (type 8.0)
    const [tailX, tailY, tailZ] = localToWorld(-2.0, 0.0, 0.0);
    const tailWiggle = breathing * 1.5;
    writePart(8, tailX, tailY, tailZ, 0, 0, curRot + tailWiggle, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 8.0);

    // 9. Mouth (type 9.0)
    const chewingOffset = isEating ? Math.sin(time * 15) * 0.08 : 0.0;
    const [mouthX, mouthY, mouthZ] = localToWorld(2.3, -0.4 + chewingOffset, 0.0);
    writePart(9, mouthX, mouthY, mouthZ, 0, 0, curRot, 0.3, 0.2, 0.3, 0.9, 0.7, 0.7, 9.0);

    return data;
}

interface SingleRabbitMeshProps {
    useMockAnimation: boolean;
    speed: number;
    rotationSpeed: number;
    isEating: boolean;
    hungerState: boolean;
    onError: (err: any) => void;
}

function SingleRabbitMesh({ useMockAnimation, speed, rotationSpeed, isEating, hungerState, onError }: SingleRabbitMeshProps) {
    const groupRef = useRef<THREE.Group>(null);
    const bodyRef = useRef<THREE.Mesh>(null);
    const headRef = useRef<THREE.Mesh>(null);
    const leftEarRef = useRef<THREE.Mesh>(null);
    const rightEarRef = useRef<THREE.Mesh>(null);
    const frontLeftLegRef = useRef<THREE.Mesh>(null);
    const frontRightLegRef = useRef<THREE.Mesh>(null);
    const hindLeftLegRef = useRef<THREE.Mesh>(null);
    const hindRightLegRef = useRef<THREE.Mesh>(null);
    const tailRef = useRef<THREE.Mesh>(null);
    const mouthRef = useRef<THREE.Mesh>(null);
    const leftEyeRef = useRef<THREE.Mesh>(null);
    const rightEyeRef = useRef<THREE.Mesh>(null);

    const latestFloatArrayRef = useRef<Float32Array | null>(null);
    const fetchingRef = useRef<boolean>(false);

    const updateRabbitTransformations = (floatArray: Float32Array, time: number) => {
        const numParts = Math.floor(floatArray.length / 13);
        if (numParts === 0) return; // Gracefully handle empty buffers

        const applyPartData = (
            mesh: THREE.Mesh | null,
            x: number, y: number, z: number,
            rotX: number, rotY: number, rotZ: number,
            scaleX: number, scaleY: number, scaleZ: number,
            r: number, g: number, b: number
        ) => {
            if (!mesh) return;
            mesh.position.set(x, y, z);
            mesh.rotation.set(rotX, rotY, rotZ);
            mesh.scale.set(scaleX, scaleY, scaleZ);
            if (mesh.material && 'color' in mesh.material) {
                (mesh.material as THREE.MeshStandardMaterial).color.setRGB(r, g, b);
            }
        };

        const refs = [
            bodyRef,           // 0
            headRef,           // 1
            leftEarRef,        // 2
            rightEarRef,       // 3
            frontLeftLegRef,   // 4
            frontRightLegRef,  // 5
            hindLeftLegRef,    // 6
            hindRightLegRef,   // 7
            tailRef,           // 8
            mouthRef           // 9
        ];

        for (let i = 0; i < numParts; i++) {
            const offset = i * 13;
            const x = floatArray[offset];
            let y = floatArray[offset + 1];
            const z = floatArray[offset + 2];
            const rotX = floatArray[offset + 3];
            const rotY = floatArray[offset + 4];
            const rotZ = floatArray[offset + 5];
            const scaleX = floatArray[offset + 6];
            const scaleY = floatArray[offset + 7];
            const scaleZ = floatArray[offset + 8];
            const r = floatArray[offset + 9];
            const g = floatArray[offset + 10];
            const b = floatArray[offset + 11];
            const partType = floatArray[offset + 12];

            let targetMesh: THREE.Mesh | null = null;
            if (partType === 7.0 && (i === 8 || i === 9 || i === 10 || i === 11)) {
                if (i === 8 || i === 10) {
                    targetMesh = leftEyeRef.current;
                } else if (i === 9 || i === 11) {
                    targetMesh = rightEyeRef.current;
                }
            } else {
                if (i < refs.length) {
                    targetMesh = refs[i].current;
                }
            }

            if (targetMesh) {
                if (useMockAnimation && targetMesh === mouthRef.current && isEating) {
                    y += Math.sin(time * 15) * 0.08;
                }
                applyPartData(targetMesh, x, y, z, rotX, rotY, rotZ, scaleX, scaleY, scaleZ, r, g, b);
            }
        }
    };

    // Tauri-based continuous polling loop
    useEffect(() => {
        if (useMockAnimation) return;

        let active = true;
        let animationFrameId: number;

        const fetchAndRenderRabbit = async () => {
            if (!active) return;
            if (fetchingRef.current) {
                animationFrameId = requestAnimationFrame(fetchAndRenderRabbit);
                return;
            }

            fetchingRef.current = true;
            try {
                const buffer = await invokeTauri<any>('get_test_rabbit_state');
                if (!active) return;
                if (!buffer) {
                    fetchingRef.current = false;
                    animationFrameId = requestAnimationFrame(fetchAndRenderRabbit);
                    return;
                }

                let floatArray: Float32Array;
                if (buffer instanceof Uint8Array) {
                    if (buffer.byteLength === 0) {
                        fetchingRef.current = false;
                        animationFrameId = requestAnimationFrame(fetchAndRenderRabbit);
                        return;
                    }
                    if (buffer.byteOffset % 4 !== 0) {
                        const alignedBuffer = buffer.slice().buffer;
                        floatArray = new Float32Array(alignedBuffer);
                    } else {
                        floatArray = new Float32Array(buffer.buffer, buffer.byteOffset, buffer.byteLength / 4);
                    }
                } else if (buffer instanceof ArrayBuffer) {
                    if (buffer.byteLength === 0) {
                        fetchingRef.current = false;
                        animationFrameId = requestAnimationFrame(fetchAndRenderRabbit);
                        return;
                    }
                    floatArray = new Float32Array(buffer);
                } else {
                    if (buffer.byteLength === 0) {
                        fetchingRef.current = false;
                        animationFrameId = requestAnimationFrame(fetchAndRenderRabbit);
                        return;
                    }
                    floatArray = new Float32Array(buffer);
                }
                latestFloatArrayRef.current = floatArray;
                updateRabbitTransformations(floatArray, 0);
            } catch (error) {
                console.warn("Unable to fetch Tauri state, switching to browser simulation:", error);
                onError(error);
            } finally {
                fetchingRef.current = false;
                if (active) {
                    animationFrameId = requestAnimationFrame(fetchAndRenderRabbit);
                }
            }
        };

        fetchAndRenderRabbit();

        return () => {
            active = false;
            cancelAnimationFrame(animationFrameId);
        };
    }, [useMockAnimation, onError]);

    // Handle hungerState eye colors
    useEffect(() => {
        const leftEye = leftEyeRef.current;
        const rightEye = rightEyeRef.current;
        const color = hungerState ? new THREE.Color(1.0, 0.0, 0.0) : new THREE.Color(0.118, 0.161, 0.231); // #EF4444 / #1E293B
        if (leftEye && leftEye.material && 'color' in leftEye.material) {
            (leftEye.material as any).color.setRGB(color.r, color.g, color.b);
        }
        if (rightEye && rightEye.material && 'color' in rightEye.material) {
            (rightEye.material as any).color.setRGB(color.r, color.g, color.b);
        }
    }, [hungerState]);

    // Animation loop
    useFrame((state) => {
        const time = state.clock.getElapsedTime();

        // Rotate the entire group mesh subtly
        if (groupRef.current) {
            if (useMockAnimation && rotationSpeed > 0) {
                groupRef.current.rotation.y = time * rotationSpeed * 0.5;
            } else {
                groupRef.current.rotation.y = 0;
            }
        }

        // Apply interactive browser-only animation OR fallback if Tauri failed
        if (useMockAnimation) {
            const floatArray = generateSingleRabbitJS(0, 0, 0.2, time, speed, isEating);
            latestFloatArrayRef.current = floatArray;
            updateRabbitTransformations(floatArray, time);
        } else if (latestFloatArrayRef.current) {
            updateRabbitTransformations(latestFloatArrayRef.current, time);
        }
    });

    // Determine eye color based on hungerState (normal: #1E293B, hungry: #EF4444)
    const eyeColor = hungerState ? "#EF4444" : "#1E293B";

    return (
        <group ref={groupRef} name="rabbit-group">
            {/* 1. Body Mesh */}
            <mesh ref={bodyRef} name="rabbit-body" castShadow receiveShadow geometry={geomBody}>
                <meshStandardMaterial roughness={0.65} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesBody}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
            </mesh>
 
            {/* 2. Head Mesh with Eyes, Blush, Snout and Nose */}
            <mesh ref={headRef} name="rabbit-head" castShadow receiveShadow geometry={geomHead}>
                <meshStandardMaterial roughness={0.65} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesHead}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
   
                {/* Snout/Muzzle (overlapping spheres) */}
                <mesh position={[0.5, -0.08, 0.08]} scale={[0.12, 0.12, 0.12]} name="rabbit-muzzle-left" geometry={geomSnout}>
                    <meshStandardMaterial roughness={0.7} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesSnout}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
                <mesh position={[0.5, -0.08, -0.08]} scale={[0.12, 0.12, 0.12]} name="rabbit-muzzle-right" geometry={geomSnout}>
                    <meshStandardMaterial roughness={0.7} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesSnout}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>

                {/* Cheek Blush (cute pink circles) */}
                <mesh position={[0.36, -0.12, 0.44]} scale={[0.09, 0.09, 0.02]} name="rabbit-blush-left" geometry={geomBlush}>
                    <meshBasicMaterial color="#FFC2CD" opacity={0.75} />
                    <lineSegments geometry={edgesBlush}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
                <mesh position={[0.36, -0.12, -0.44]} scale={[0.09, 0.09, 0.02]} name="rabbit-blush-right" geometry={geomBlush}>
                    <meshBasicMaterial color="#FFC2CD" opacity={0.75} />
                    <lineSegments geometry={edgesBlush}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>


                {/* Two eye spheres */}
                <mesh ref={leftEyeRef} position={[0.35, 0.15, 0.35]} name="rabbit-eye-left" geometry={geomEye}>
                    <meshStandardMaterial color={eyeColor} roughness={0.1} metalness={0.9} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesEye}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                    {/* Eye Glints */}
                    <mesh position={[0.045, 0.045, 0.045]} scale={[0.26, 0.26, 0.26]} geometry={geomEyeGlint}>
                        <meshBasicMaterial color="#FFFFFF" />
                        <lineSegments geometry={edgesEyeGlint}>
                            <lineBasicMaterial color="#1e293b" />
                        </lineSegments>
                    </mesh>
                    <mesh position={[0.07, -0.02, 0.02]} scale={[0.13, 0.13, 0.13]} geometry={geomEyeGlint}>
                        <meshBasicMaterial color="#FFFFFF" />
                        <lineSegments geometry={edgesEyeGlint}>
                            <lineBasicMaterial color="#1e293b" />
                        </lineSegments>
                    </mesh>
                </mesh>
                <mesh ref={rightEyeRef} position={[0.35, 0.15, -0.35]} name="rabbit-eye-right" geometry={geomEye}>
                    <meshStandardMaterial color={eyeColor} roughness={0.1} metalness={0.9} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesEye}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                    {/* Eye Glints */}
                    <mesh position={[0.045, 0.045, -0.045]} scale={[0.26, 0.26, 0.26]} geometry={geomEyeGlint}>
                        <meshBasicMaterial color="#FFFFFF" />
                        <lineSegments geometry={edgesEyeGlint}>
                            <lineBasicMaterial color="#1e293b" />
                        </lineSegments>
                    </mesh>
                    <mesh position={[0.07, -0.02, -0.02]} scale={[0.13, 0.13, 0.13]} geometry={geomEyeGlint}>
                        <meshBasicMaterial color="#FFFFFF" />
                        <lineSegments geometry={edgesEyeGlint}>
                            <lineBasicMaterial color="#1e293b" />
                        </lineSegments>
                    </mesh>
                </mesh>

   
                {/* Small pink nose sphere */}
                <mesh position={[0.58, -0.01, 0]} name="rabbit-nose" geometry={geomNose}>
                    <meshStandardMaterial color="#FDA4AF" roughness={0.4} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesNose}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
            </mesh>
 
            {/* 3. Left Ear Mesh with Pink Inner Ear overlay */}
            <mesh ref={leftEarRef} name="rabbit-left-ear" castShadow receiveShadow geometry={geomEar}>
                <meshStandardMaterial roughness={0.65} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesEar}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
   
                {/* Pink inner ear overlay mesh */}
                <mesh position={[0.05, 0.02, 0.03]} scale={[0.9, 0.8, 0.2]} name="rabbit-left-ear-inner" geometry={geomEar}>
                    <meshStandardMaterial color="#FDA4AF" roughness={0.6} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesEar}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
            </mesh>
 
            {/* 4. Right Ear Mesh with Pink Inner Ear overlay */}
            <mesh ref={rightEarRef} name="rabbit-right-ear" castShadow receiveShadow geometry={geomEar}>
                <meshStandardMaterial roughness={0.65} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesEar}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
   
                {/* Pink inner ear overlay mesh */}
                <mesh position={[0.05, 0.02, -0.03]} scale={[0.9, 0.8, 0.2]} name="rabbit-right-ear-inner" geometry={geomEar}>
                    <meshStandardMaterial color="#FDA4AF" roughness={0.6} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesEar}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
            </mesh>
 
            {/* 5. Front-Left Leg Mesh */}
            <mesh ref={frontLeftLegRef} name="rabbit-front-left-leg" castShadow receiveShadow geometry={geomLeg}>
                <meshStandardMaterial roughness={0.6} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesLeg}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
            </mesh>

            {/* 6. Front-Right Leg Mesh */}
            <mesh ref={frontRightLegRef} name="rabbit-front-right-leg" castShadow receiveShadow geometry={geomLeg}>
                <meshStandardMaterial roughness={0.6} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesLeg}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
            </mesh>
 
            {/* 7. Hind-Left Leg Mesh */}
            <mesh ref={hindLeftLegRef} name="rabbit-hind-left-leg" castShadow receiveShadow geometry={geomLeg}>
                <meshStandardMaterial roughness={0.6} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesLeg}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
            </mesh>
 
            {/* 8. Hind-Right Leg Mesh */}
            <mesh ref={hindRightLegRef} name="rabbit-hind-right-leg" castShadow receiveShadow geometry={geomLeg}>
                <meshStandardMaterial roughness={0.6} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesLeg}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
            </mesh>
 
            {/* 9. Tail Mesh (Fluffy Cloud structure) */}
            <mesh ref={tailRef} name="rabbit-tail" castShadow geometry={geomTail}>
                <meshStandardMaterial roughness={0.9} metalness={0.0} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesTail}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
                {/* Fluffy tail overlays */}
                <mesh position={[-0.38, -0.25, 0.44]} scale={[0.56, 0.56, 0.56]} castShadow geometry={geomTail}>
                    <meshStandardMaterial color="#FFFFFF" roughness={0.9} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesTail}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
                <mesh position={[-0.38, -0.25, -0.44]} scale={[0.56, 0.56, 0.56]} castShadow geometry={geomTail}>
                    <meshStandardMaterial color="#FFFFFF" roughness={0.9} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesTail}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
                <mesh position={[0.06, 0.44, 0]} scale={[0.62, 0.62, 0.62]} castShadow geometry={geomTail}>
                    <meshStandardMaterial color="#FFFFFF" roughness={0.9} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                    <lineSegments geometry={edgesTail}>
                        <lineBasicMaterial color="#1e293b" />
                    </lineSegments>
                </mesh>
            </mesh>
 
            {/* 10. Mouth Mesh */}
            <mesh ref={mouthRef} name="rabbit-jaw" castShadow geometry={geomMouth}>
                <meshStandardMaterial roughness={0.7} metalness={0.1} flatShading={true} polygonOffset={true} polygonOffsetFactor={1} polygonOffsetUnits={1} />
                <lineSegments geometry={edgesMouth}>
                    <lineBasicMaterial color="#1e293b" />
                </lineSegments>
            </mesh>
        </group>
    );
}

export default function RabbitVisualizer() {
    const [useMock, setUseMock] = useState<boolean>(true);
    const [tauriError, setTauriError] = useState<boolean>(false);
    const [speed, setSpeed] = useState<number>(1.2);
    const [rotationSpeed, setRotationSpeed] = useState<number>(0.2);
    const [isEating, setIsEating] = useState<boolean>(false);
    const [hungerState, setHungerState] = useState<boolean>(false);

    const handleTauriError = () => {
        setTauriError(true);
        setUseMock(true); // Auto-fallback
    };

    return (
        <div style={{
            display: 'flex',
            flexDirection: 'column',
            width: '100%',
            backgroundColor: '#1E293B',
            borderRadius: '16px',
            overflow: 'hidden',
            fontFamily: 'system-ui, -apple-system, sans-serif',
            boxShadow: '0 10px 25px -5px rgba(0, 0, 0, 0.3), 0 8px 10px -6px rgba(0, 0, 0, 0.3)',
            border: '1px solid rgba(255, 255, 255, 0.05)'
        }}>
            {/* Header with glassmorphism */}
            <div style={{
                padding: '16px 24px',
                background: 'linear-gradient(135deg, #3B82F6 0%, #8B5CF6 100%)',
                color: 'white',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                flexWrap: 'wrap',
                gap: '12px'
            }}>
                <div>
                    <h3 style={{ margin: 0, fontSize: '18px', fontWeight: 600, letterSpacing: '0.5px', display: 'flex', alignItems: 'center', gap: '8px' }}>
                        🐰 Rabbit Procedural Morphology Visualizer
                    </h3>
                    <p style={{ margin: '4px 0 0 0', fontSize: '12px', opacity: 0.9 }}>
                        {useMock ? "Running in Web Sandbox mode (No Rust Backend Required)" : "Connected to Live Tauri Rust Simulator"}
                    </p>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                    <button
                        onClick={() => {
                            if (!tauriError) {
                                setUseMock(false);
                            }
                        }}
                        disabled={tauriError}
                        style={{
                            padding: '6px 12px',
                            fontSize: '12px',
                            fontWeight: 500,
                            borderRadius: '20px',
                            border: 'none',
                            cursor: tauriError ? 'not-allowed' : 'pointer',
                            backgroundColor: !useMock ? '#FFFFFF' : 'rgba(255, 255, 255, 0.15)',
                            color: !useMock ? '#1E293B' : '#FFFFFF',
                            transition: 'all 0.2s',
                            opacity: tauriError ? 0.5 : 1
                        }}
                    >
                        Live Tauri State
                    </button>
                    <button
                        onClick={() => setUseMock(true)}
                        style={{
                            padding: '6px 12px',
                            fontSize: '12px',
                            fontWeight: 500,
                            borderRadius: '20px',
                            border: 'none',
                            cursor: 'pointer',
                            backgroundColor: useMock ? '#FFFFFF' : 'rgba(255, 255, 255, 0.15)',
                            color: useMock ? '#1E293B' : '#FFFFFF',
                            transition: 'all 0.2s'
                        }}
                    >
                        Interactive Web Mock
                    </button>
                </div>
            </div>

            {/* Main simulator content */}
            <div style={{ display: 'flex', flexDirection: 'row', flexWrap: 'wrap', minHeight: '400px' }}>
                {/* 3D Canvas rendering panel */}
                <div style={{ flex: '1 1 350px', height: '400px', position: 'relative', backgroundColor: '#0F172A' }}>
                    {/* Floating Status Badge */}
                    <div style={{
                        position: 'absolute',
                        top: '12px',
                        left: '12px',
                        zIndex: 10,
                        backgroundColor: useMock ? 'rgba(59, 130, 246, 0.2)' : 'rgba(16, 185, 129, 0.2)',
                        backdropFilter: 'blur(8px)',
                        border: useMock ? '1px solid rgba(59, 130, 246, 0.4)' : '1px solid rgba(16, 185, 129, 0.4)',
                        color: useMock ? '#60A5FA' : '#34D399',
                        padding: '4px 10px',
                        borderRadius: '6px',
                        fontSize: '11px',
                        fontWeight: 600,
                        textTransform: 'uppercase',
                        letterSpacing: '1px'
                    }}>
                        {useMock ? "Web Sandbox" : "Tauri Live"}
                    </div>

                    <Canvas shadows camera={{ position: [0, 1.5, 5], fov: 45 }}>
                        {/* Soft ambient light */}
                        <ambientLight intensity={0.8} color="#1E1B4B" />
                        {/* Warm Key Light */}
                        <directionalLight
                            position={[4, 7, 3]}
                            intensity={1.6}
                            castShadow
                            shadow-mapSize-width={1024}
                            shadow-mapSize-height={1024}
                            shadow-bias={-0.002}
                        />
                        {/* Cool Fill Light */}
                        <directionalLight position={[-4, 2, 2]} intensity={0.6} color="#38BDF8" />
                        {/* Rim Light */}
                        <spotLight position={[-3, 5, -4]} intensity={2.5} color="#F472B6" angle={Math.PI / 4} />
                        
                        {/* Showcase Pedestal */}
                        <group position={[0, -0.52, 0]}>
                            <mesh receiveShadow>
                                <cylinderGeometry args={[1.6, 1.7, 0.16, 8]} />
                                <meshStandardMaterial color="#1e1b4b" roughness={0.3} metalness={0.7} flatShading={true} />
                            </mesh>
                            <mesh position={[0, 0.08, 0]}>
                                <cylinderGeometry args={[1.62, 1.62, 0.04, 8]} />
                                <meshStandardMaterial color="#fbbf24" roughness={0.1} metalness={0.9} flatShading={true} />
                            </mesh>
                        </group>

                        <SingleRabbitMesh
                            useMockAnimation={useMock}
                            speed={speed}
                            rotationSpeed={rotationSpeed}
                            isEating={isEating}
                            hungerState={hungerState}
                            onError={handleTauriError}
                        />
                    </Canvas>
                </div>

                {/* Sidebar controls */}
                <div style={{
                    width: '300px',
                    padding: '24px',
                    backgroundColor: '#1E293B',
                    borderLeft: '1px solid rgba(255, 255, 255, 0.05)',
                    display: 'flex',
                    flexDirection: 'column',
                    justifyContent: 'space-between',
                    gap: '20px'
                }}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                        <h4 style={{ margin: 0, color: '#F8FAFC', fontSize: '14px', fontWeight: 600, letterSpacing: '0.5px' }}>
                            SIMULATOR CONTROLS
                        </h4>

                        {/* Speed slider */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', color: '#94A3B8' }}>
                                <span>Hop & Breath Speed</span>
                                <span style={{ color: '#60A5FA', fontWeight: 'bold' }}>{speed.toFixed(1)}x</span>
                            </div>
                            <input
                                type="range"
                                min="0.2"
                                max="3.0"
                                step="0.1"
                                value={speed}
                                onChange={(e) => setSpeed(parseFloat(e.target.value))}
                                disabled={!useMock}
                                style={{
                                    width: '100%',
                                    accentColor: '#3B82F6',
                                    cursor: useMock ? 'pointer' : 'not-allowed',
                                    opacity: useMock ? 1 : 0.4
                                }}
                            />
                        </div>

                        {/* Orbit / Rotation speed slider */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', color: '#94A3B8' }}>
                                <span>Y-Axis Rotation (3D)</span>
                                <span style={{ color: '#8B5CF6', fontWeight: 'bold' }}>{rotationSpeed.toFixed(1)}x</span>
                            </div>
                            <input
                                type="range"
                                min="0.0"
                                max="1.5"
                                step="0.1"
                                value={rotationSpeed}
                                onChange={(e) => setRotationSpeed(parseFloat(e.target.value))}
                                style={{
                                    width: '100%',
                                    accentColor: '#8B5CF6',
                                    cursor: 'pointer'
                                }}
                            />
                        </div>

                        {/* Toggle isEating */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: '12px', color: '#94A3B8' }}>
                            <span>Chewing Animation (isEating)</span>
                            <button
                                onClick={() => setIsEating(!isEating)}
                                style={{
                                    padding: '6px 12px',
                                    fontSize: '11px',
                                    fontWeight: 600,
                                    borderRadius: '6px',
                                    border: 'none',
                                    cursor: 'pointer',
                                    backgroundColor: isEating ? '#10B981' : '#475569',
                                    color: '#FFFFFF',
                                    transition: 'all 0.2s'
                                }}
                            >
                                {isEating ? 'EATING' : 'IDLE'}
                            </button>
                        </div>

                        {/* Toggle hungerState */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: '12px', color: '#94A3B8' }}>
                            <span>Hunger State (Red Eyes)</span>
                            <button
                                onClick={() => setHungerState(!hungerState)}
                                style={{
                                    padding: '6px 12px',
                                    fontSize: '11px',
                                    fontWeight: 600,
                                    borderRadius: '6px',
                                    border: 'none',
                                    cursor: 'pointer',
                                    backgroundColor: hungerState ? '#EF4444' : '#475569',
                                    color: '#FFFFFF',
                                    transition: 'all 0.2s'
                                }}
                            >
                                {hungerState ? 'HUNGRY' : 'SATISFIED'}
                            </button>
                        </div>
                    </div>

                    {/* Morphology Specs Info card */}
                    <div style={{
                        padding: '12px 16px',
                        backgroundColor: '#151E2D',
                        borderRadius: '10px',
                        border: '1px solid rgba(255, 255, 255, 0.03)'
                    }}>
                        <h5 style={{ margin: '0 0 8px 0', color: '#F1F5F9', fontSize: '11px', fontWeight: 600, letterSpacing: '0.5px', textTransform: 'uppercase' }}>
                            Procedural Part Breakdown
                        </h5>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', fontSize: '11px' }}>
                            <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8' }}>
                                <span>⚪ Body (0.0):</span>
                                <span style={{ color: '#E2E8F0', fontFamily: 'monospace' }}>2.0x Scale Ellipse</span>
                            </div>
                            <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8' }}>
                                <span>⚪ Head (1.0):</span>
                                <span style={{ color: '#E2E8F0', fontFamily: 'monospace' }}>1.2x Scale Sphere</span>
                            </div>
                            <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8' }}>
                                <span>⚪ Ears (2.0/3.0):</span>
                                <span style={{ color: '#E2E8F0', fontFamily: 'monospace' }}>2.8x Scale Long Spanning</span>
                            </div>
                            <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8' }}>
                                <span>⚪ Legs (4.0-7.0):</span>
                                <span style={{ color: '#E2E8F0', fontFamily: 'monospace' }}>1.4x Scale Muscles</span>
                            </div>
                            <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8' }}>
                                <span>⚪ Tail (8.0):</span>
                                <span style={{ color: '#E2E8F0', fontFamily: 'monospace' }}>0.5x Scale Fluff</span>
                            </div>
                            <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8' }}>
                                <span>⚪ Mouth (9.0):</span>
                                <span style={{ color: '#E2E8F0', fontFamily: 'monospace' }}>0.3x Scale Jaw</span>
                            </div>
                        </div>
                    </div>

                </div>
            </div>

            {/* Info bar at the bottom */}
            {tauriError && (
                <div style={{
                    padding: '8px 16px',
                    backgroundColor: 'rgba(245, 158, 11, 0.1)',
                    borderTop: '1px solid rgba(245, 158, 11, 0.2)',
                    color: '#F59E0B',
                    fontSize: '11px',
                    textAlign: 'center',
                    fontWeight: 500
                }}>
                    ⚠️ Rust Tauri Backend is not active. The app automatically fell back to the interactive high-performance Web Sandbox.
                </div>
            )}
        </div>
    );
}
