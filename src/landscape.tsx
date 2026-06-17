import React from "react";
import ReactDOM from "react-dom/client";
import { LandscapeShowcase } from "./components/Landscape/LandscapeShowcase";

const rootEl = document.getElementById("root");
if (rootEl) {
  ReactDOM.createRoot(rootEl).render(
    <React.StrictMode>
      <div style={{ width: "100vw", height: "100vh", margin: 0, padding: 0, overflow: "hidden" }}>
        <LandscapeShowcase />
      </div>
    </React.StrictMode>
  );
}
