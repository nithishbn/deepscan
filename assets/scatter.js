function initScatter() {
  // set the dimensions and margins of the graph
  var margin = { top: 10, right: 30, bottom: 30, left: 60 },
    width = 460 - margin.left - margin.right,
    height = 400 - margin.top - margin.bottom;

  // append the svg object to the body of the page
  var svg = d3
    .select("#container")
    .append("svg")
    .attr("width", width + margin.left + margin.right)
    .attr("height", height + margin.top + margin.bottom)
    .append("g")
    .attr("transform", "translate(" + margin.left + "," + margin.top + ")");

  // Add X axis
  var x = d3.scaleLinear().domain([-5, 5]).range([0, width]);
  svg
    .append("g")
    .attr("transform", "translate(0," + height + ")")
    .call(d3.axisBottom(x));

  // Add Y axis
  var y = d3.scaleLinear().domain([0, 1]).range([height, 0]);
  svg.append("g").call(d3.axisLeft(y));
  return { svg, x, y };
}

function addPoints(data) {
  const { svg, x, y } = initScatter();
  // Add dots
  svg
    .append("g")
    .selectAll("dot")
    .data(data)
    .enter()
    .append("circle")
    .attr("cx", function (d) {
      return x(d.log2_fold_change);
    })
    .attr("cy", function (d) {
      return y(d.p_value);
    })
    .attr("r", 1.5)
    .style("fill", function (d) {
      return d.color;
    });
}
