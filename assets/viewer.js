function create_viewer() {
  return molstar.Viewer.create("structure", {
    layoutIsExpanded: false,
    layoutShowControls: false,
    layoutShowRemoteState: false,
    layoutShowSequence: true,
    layoutShowLog: false,
    layoutShowLeftPanel: true,
    transparency: 0.5,
    viewportShowExpand: false,
    viewportShowSelectionMode: false,
    viewportShowAnimation: true,

    pdbProvider: "rcsb",
    emdbProvider: "rcsb",
  });
}
var viewer = create_viewer();

function refresh_and_load_pdb_into_viewer(pdb_id) {
  // console.log("hello");
  viewer.then((viewer) => {
    viewer.plugin.clear();
    console.log(pdb_id);
    viewer.loadPdb(pdb_id);
  });
}
