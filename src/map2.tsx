import React from "react";
import ReactDOM from "react-dom/client";
import { Map2Showcase } from "./components/Map2/Map2Showcase";

const rootEl = document.getElementById("root");
if (rootEl) {
  ReactDOM.createRoot(rootEl).render(
    <React.StrictMode>
      <div style={{ width: "100vw", height: "100vh", margin: 0, padding: 0, overflow: "hidden", background: "#000" }}>
        <Map2Showcase isStandalone={true} />
      </div>
    </React.StrictMode>
  );
}
