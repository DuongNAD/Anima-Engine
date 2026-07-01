import React from "react";
import ReactDOM from "react-dom/client";
import { WorldShowcase } from "./components/Landscape/WorldShowcase";

const rootEl = document.getElementById("root");
if (rootEl) {
  ReactDOM.createRoot(rootEl).render(
    <React.StrictMode>
      <div style={{ width: "100vw", height: "100vh", margin: 0, padding: 0, overflow: "hidden" }}>
        <WorldShowcase />
      </div>
    </React.StrictMode>
  );
}
