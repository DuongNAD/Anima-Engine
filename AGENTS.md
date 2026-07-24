<!-- BEGIN ANIMAL MAP VISION -->
# Animal Map Vision MCP — required workflow

For any task involving world generation, terrain, biomes, ecosystems, vegetation,
animals, navigation, collisions, water, lighting, map screenshots, or map quality:

1. Read and follow the `animal-map-review` skill.
2. Confirm the `animal-map-vision` MCP is available. If it is missing, do not claim the
   map was reviewed; ask the user to run `npm run doctor -- --project <project-path>`
   from the MCP repository and reload `/mcp`.
3. Call tools in this order:
   - `discover_map_artifacts`
   - `validate_map_manifest`
   - `prepare_team_review`
   - `inspect_map_views`
4. Treat manifest results as hard gates. Every visual finding must cite an image path
   and region. Separate observed evidence from hypotheses.
5. Do not claim map completion until validation passes with no critical/high finding,
   canonical before/after views were inspected, navigation is reachable, and ecological
   contradictions are resolved.

For `/teamwork-preview`, use the roles Geometry & Collision, Navigation & Gameplay,
Ecology & World Coherence, Visual & Lighting, and Integrator. The Integrator owns the
final evidence table and reruns all gates.
<!-- END ANIMAL MAP VISION -->
