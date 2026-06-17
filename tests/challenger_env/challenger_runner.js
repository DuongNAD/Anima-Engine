const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

async function run() {
  console.log("Starting visual verification and empirical tests...");
  
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  
  // Set viewport to a good resolution
  await page.setViewportSize({ width: 1280, height: 720 });
  
  const consoleErrors = [];
  const consoleLogs = [];
  
  page.on('pageerror', exception => {
    consoleErrors.push({ type: 'pageerror', text: exception.message, stack: exception.stack });
    console.error("PAGE ERROR:", exception.message);
  });
  
  page.on('console', msg => {
    const text = msg.text();
    const type = msg.type();
    consoleLogs.push({ type, text });
    if (type === 'error') {
      consoleErrors.push({ type: 'console-error', text });
      console.error("CONSOLE ERROR:", text);
    } else {
      console.log(`[Browser Console ${type}] ${text}`);
    }
  });

  // Intercept ecosystem.html to expose internal states safely without breaking variable scoping
  await page.route('**/ecosystem.html', async route => {
    try {
      const response = await route.fetch();
      let body = await response.text();
      
      // Safe injection: keep original variables intact, just attach references to window
      body = body.replace('let mapReady=false;', 'let mapReady=false; window.mapReady=false;');
      body = body.replace(/mapReady=true;/g, 'mapReady=true; window.mapReady=true;');
      body = body.replace('const waterBodies=[];', 'const waterBodies=[]; window.waterBodies=waterBodies;');
      body = body.replace('const lakes = [];', 'const lakes = []; window.lakes=lakes;');
      
      // Fix terrElev replacement
      const targetElevDecl = 'const terrElev=new Float32Array(TOT),moisture=new Float32Array(TOT),temperature=new Float32Array(TOT);';
      const replacementElevDecl = 'const terrElev=new Float32Array(TOT),moisture=new Float32Array(TOT),temperature=new Float32Array(TOT); window.terrElev=terrElev;';
      body = body.replace(targetElevDecl, replacementElevDecl);
      
      // Expose orbTarget on window to allow camera manipulation
      body = body.replace(
        'const orbTarget=new THREE.Vector3(0,25,0);',
        'const orbTarget=new THREE.Vector3(0,25,0); window.orbTarget=orbTarget;'
      );
      
      await route.fulfill({
        response,
        body,
        headers: {
          ...response.headers(),
          'content-type': 'text/html',
        }
      });
    } catch (err) {
      console.error("Route interception failed:", err);
      route.continue();
    }
  });

  try {
    console.log("Navigating to http://localhost:5173/ecosystem.html...");
    await page.goto('http://localhost:5173/ecosystem.html', { waitUntil: 'load', timeout: 30000 });
    
    console.log("Waiting for mapReady to become true...");
    await page.waitForFunction(() => window.mapReady === true, { timeout: 20000 });
    
    // Additional delay to ensure rendering and physics stabilize
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    console.log("Extracting client-side states...");
    const data = await page.evaluate(() => {
      return {
        lakes: window.lakes,
        waterBodiesCount: window.waterBodies.length,
        waterBodies: window.waterBodies.map(wb => ({
          type: wb.type,
          y: wb.mesh ? wb.mesh.position.y : null,
          baseY: wb.baseY
        })),
        terrElev: Array.from(window.terrElev)
      };
    });

    console.log("--- Empirical Test Results ---");
    console.log(`Lakes generated count: ${data.lakes.length}`);
    console.log(`Water bodies registered count: ${data.waterBodiesCount}`);
    
    let lakeLevelCheckPassed = true;
    const lakeLevelViolations = [];
    data.lakes.forEach((lk, idx) => {
      console.log(`Lake ${idx}: Center=(${lk.x}, ${lk.y}), Radius=${lk.r}, Water level (y)=${lk.waterY.toFixed(2)}`);
      if (lk.waterY >= 30) {
        lakeLevelCheckPassed = false;
        lakeLevelViolations.push({ index: idx, ...lk });
      }
    });

    // Check mountain peaks (elevation >= 100)
    const GW = 200;
    const GH = 200;
    const peaks = [];
    for (let y = 0; y < GH; y++) {
      for (let x = 0; x < GW; x++) {
        const elev = data.terrElev[y * GW + x];
        if (elev >= 100) {
          peaks.push({ x, y, elev });
        }
      }
    }
    console.log(`Detected snow-capped peak cells (elevation >= 100): ${peaks.length}`);

    let peakProximityCheckPassed = true;
    const proximityViolations = [];
    
    data.lakes.forEach((lk, idx) => {
      let minCenterDist = Infinity;
      let closestPeak = null;
      peaks.forEach(peak => {
        const dx = lk.x - peak.x;
        const dy = lk.y - peak.y;
        const dist = Math.sqrt(dx*dx + dy*dy);
        if (dist < minCenterDist) {
          minCenterDist = dist;
          closestPeak = peak;
        }
      });
      const distToShore = minCenterDist - lk.r;
      console.log(`Lake ${idx} Shore Distance to closest Peak cell: ${distToShore.toFixed(2)} (Peak at [${closestPeak?.x}, ${closestPeak?.y}] elev=${closestPeak?.elev.toFixed(1)})`);
      
      // We check if it is near (e.g. shore distance < 10 cells)
      if (distToShore < 10) {
        peakProximityCheckPassed = false;
        proximityViolations.push({ lakeIndex: idx, distToShore, closestPeak });
      }
    });

    // Capture screenshot from a high angle looking down on the valleys
    console.log("Positioning camera for high-angle valley check...");
    await page.evaluate(() => {
      // Set camera modes and controls for beautiful high-angle shot
      orbPhi = 0.45; // high angle looking down
      orbTheta = 0.95; // angle to capture contrast between peaks and valleys
      orbDist = 380;
      
      if (window.orbTarget) {
        window.orbTarget.set(0, 15, 0);
      } else {
        orbTarget.set(0, 15, 0);
      }
    });

    // Wait for camera interpolation to finish and renderer to draw
    await new Promise(resolve => setTimeout(resolve, 2500));

    const screenshotPath = 'e:\\project\\Anima-Engine\\lake_visual_check_2.png';
    console.log(`Saving screenshot to ${screenshotPath}...`);
    await page.screenshot({ path: screenshotPath });
    console.log("Screenshot saved successfully.");

    // Evaluate summary
    console.log("\nSummary:");
    console.log(`1. Console errors count: ${consoleErrors.length}`);
    console.log(`2. Lake water level check (< 30): ${lakeLevelCheckPassed ? "PASS" : "FAIL"}`);
    if (!lakeLevelCheckPassed) {
      console.log("   Violations:", lakeLevelViolations);
    }
    console.log(`3. Peak proximity check (shore distance >= 10): ${peakProximityCheckPassed ? "PASS" : "FAIL"}`);
    if (!peakProximityCheckPassed) {
      console.log("   Violations:", proximityViolations);
    }

    const testPassed = consoleErrors.length === 0 && lakeLevelCheckPassed && peakProximityCheckPassed;
    console.log(`Overall Status: ${testPassed ? "SUCCESS" : "FAILED"}`);

    // Output a JSON file with test results for easy ingestion by challenger agent
    const results = {
      timestamp: new Date().toISOString(),
      testPassed,
      consoleErrorsCount: consoleErrors.length,
      consoleErrors,
      lakeLevelCheckPassed,
      lakeLevelViolations,
      peakProximityCheckPassed,
      proximityViolations,
      lakes: data.lakes,
      waterBodiesCount: data.waterBodiesCount
    };

    fs.writeFileSync(
      path.resolve(__dirname, 'challenger_results.json'),
      JSON.stringify(results, null, 2)
    );
    console.log("Results written to challenger_results.json");

  } catch (err) {
    console.error("Test runner crashed:", err);
  } finally {
    await browser.close();
    console.log("Browser closed.");
  }
}

run();
