// Declare a global variable for the viewer instance
let viewerInstance = null;
const options = {
  hideControls: true,
  bgColor: { r: 255, g: 255, b: 255 },
};
function initMolstar() {
  viewerInstance = new PDBeMolstarPlugin();

  const viewerContainer = document.getElementById("structure");
  viewerInstance.visual.visibility({ water: false });

  viewerInstance.render(viewerContainer, {
    moleculeId: "1cbs",
    ...options,
  });

  viewerInstance.events.loadComplete.subscribe(() => {
    console.log("Molstar viewer loaded.");
  });
}
function refresh_and_load_pdb_into_viewer(pdb_id) {
  viewerInstance.visual.update({ ...options, moleculeId: pdb_id }, true);
}

function colorVariants(variants) {
  // Prepare the data array to color the variants
  console.log(variants);
  const variantData = variants.map((variant) => ({
    residue_number: variant.pos,
    color: variant.color,
  }));

  // Pass the array to Molstar's `visual.select`
  viewerInstance.visual.select({
    data: variantData,
    nonSelectedColor: { r: 255, g: 255, b: 255 },
  });
}
