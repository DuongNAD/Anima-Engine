// Minimal JSX typings for the React-Three-Fiber intrinsic elements used in this
// project. @react-three/fiber is aliased to a test/build mock that does not carry
// its own JSX augmentation, so these are declared here as `any`.
declare namespace JSX {
  interface IntrinsicElements {
    // Objects
    mesh: any;
    instancedMesh: any;
    group: any;
    object3D: any;
    points: any;
    line: any;
    lineSegments: any;
    lod: any;
    primitive: any;
    // Lights
    ambientLight: any;
    directionalLight: any;
    hemisphereLight: any;
    spotLight: any;
    pointLight: any;
    // Geometries
    bufferGeometry: any;
    bufferAttribute: any;
    boxGeometry: any;
    sphereGeometry: any;
    planeGeometry: any;
    cylinderGeometry: any;
    coneGeometry: any;
    circleGeometry: any;
    dodecahedronGeometry: any;
    // Materials
    meshStandardMaterial: any;
    meshBasicMaterial: any;
    pointsMaterial: any;
    shaderMaterial: any;
    lineBasicMaterial: any;
    // Scene
    fog: any;
    fogExp2: any;
    // Controls (registered via extend)
    orbitControls: any;
  }
}
