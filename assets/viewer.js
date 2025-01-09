const options = {
  moleculeId: "1cbs",
  hideControls: true,
  bgColor: { r: 255, g: 255, b: 255 },
};
const viewerInstance = new PDBeMolstarPlugin();
// Get element from HTML/Template to place the viewer
const viewerContainer = document.getElementById("structure");
viewerInstance.visual.visibility({ water: false });
// Call render method to display the 3D view
viewerInstance.render(viewerContainer, options);

// Subscribe to events
viewerInstance.events.loadComplete.subscribe(() => {
  console.log("Loaded");
});
function refresh_and_load_pdb_into_viewer(pdb_id) {
  viewerInstance.visual.update({ ...options, moleculeId: pdb_id }, true);
}
function color_variant(pos, color) {
  viewerInstance.visual.select({
    data: [{ residue_number: pos, color: color }],

    nonSelectedColor: { r: 255, g: 255, b: 255 },
  });
}
