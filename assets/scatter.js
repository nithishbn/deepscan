const point_radius = 4;

function scatterPlot() {
  return {
    svg: null,
    x: null,
    y: null,
    width: null,
    height: null,
    data: [], // The initial data for the points

    initPlot(xmin, xmax, ymin, ymax) {
      // Initialize the scatter plot once
      const { svg, x, y } = this.initScatter(xmin, xmax, ymin, ymax);
      this.svg = svg;
      this.x = x;
      this.y = y;

      this.initBrush();
    },

    initScatter(xmin, xmax, ymin, ymax) {
      var margin = { top: 20, right: 30, bottom: 40, left: 60 };

      // Get the width and height of the container dynamically
      var container = d3.select("#container");
      var width =
        parseInt(container.style("width")) - margin.left - margin.right;
      var height =
        parseInt(container.style("height")) - margin.top - margin.bottom;
      this.width = width;
      this.height = height;
      // Append the SVG object to the container
      var svg = d3
        .select("#container")
        .append("svg")
        .attr("width", width + margin.left + margin.right)
        .attr("height", height + margin.top + margin.bottom)
        .append("g")
        .attr("transform", "translate(" + margin.left + "," + margin.top + ")");

      // Add X axis
      var x = d3.scaleLinear().domain([xmin, xmax]).range([0, width]);
      svg
        .append("g")
        .attr("transform", "translate(0," + height + ")")
        .call(d3.axisBottom(x));

      // Add Y axis
      var y = d3.scaleLinear().domain([ymin, ymax]).range([height, 0]);
      svg.append("g").call(d3.axisLeft(y));
      svg
        .append("text")
        .attr(
          "transform",
          "translate(" + width / 2 + " ," + (height + margin.bottom) + ")",
        )
        .style("text-anchor", "middle")
        .text("Log2 Fold Change");

      svg
        .append("text")
        .attr("transform", "rotate(-90)")
        .attr("y", 0 - margin.left)
        .attr("x", 0 - height / 2)
        .attr("dy", "1em")
        .style("text-anchor", "middle")
        .text("-log10(p value)");
      return { svg, x, y };
    },
    // Initialize the brush for selection
    initBrush() {
      const brush = d3
        .brush()
        .extent([
          [0, 0],
          [this.width, this.height],
        ]) // The area where the brush can be used
        .on("end", this.brushed.bind(this)); // Bind 'brushed' method

      this.svg.append("g").call(brush);
    },

    // Handle the brush event
    brushed(event) {
      const selection = event.selection;
      if (!selection) {
        colorVariants([]);
        return;
      }

      const [[x0, y0], [x1, y1]] = selection;

      // Find the points within the selection area
      const selectedPoints = this.data.filter((d) => {
        const cx = this.x(d.log2_fold_change);
        const cy = this.y(d.p_value);
        return cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1;
      });
      colorVariants(selectedPoints);
      const selectedIds = selectedPoints.map((d) => d.id);
      const idsQuery = selectedIds.join(",");
      htmx.ajax("GET", `/variant?ids=${idsQuery}`, {
        target: "#variant-view-body", // update this with the target element you want to replace/update
      });
    },

    // Method to update the scatter plot points
    updatePoints() {
      if (!this.svg) return;

      // Bind the new data to the circles
      const points = this.svg.selectAll("circle").data(this.data, (d) => d.pos);

      // Remove old points with a fade-out transition
      points
        .exit()
        .transition()
        .duration(750)
        .attr("r", 0)
        .style("opacity", 0)
        .remove();

      // Update existing points
      points
        .transition()
        .duration(750)
        .attr("cx", (d) => this.x(d.log2_fold_change))
        .attr("cy", (d) => this.y(d.p_value))
        .attr("r", point_radius)
        .style("fill", (d) => d.color);

      // Add new points with a fade-in transition
      points
        .enter()
        .append("circle")
        .attr("cx", (d) => this.x(d.log2_fold_change))
        .attr("cy", (d) => this.y(d.p_value))
        .attr("r", 0) // Start with radius 0 for fade-in effect
        .style("fill", (d) => d.color)
        .style("opacity", 0) // Start with opacity 0 for fade-in effect
        .transition()
        .duration(750)
        .attr("r", point_radius)
        .style("opacity", 1);
    },

    // Method to set new data and update the points
    setData(newData) {
      this.data = newData;
      this.updatePoints(); // Re-render the points with the new data
    },
  };
}
