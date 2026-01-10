use anyhow::Result;
use plotters::prelude::*;

use crate::benchmark::BenchmarkResults;

pub fn generate_plots(results: &BenchmarkResults, output_dir: &str) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    // Plot 1: Latency comparison
    plot_latency_comparison(results, &format!("{}/latency_comparison.png", output_dir))?;

    // Plot 2: Quality by category
    plot_quality_by_category(results, &format!("{}/quality_by_category.png", output_dir))?;

    Ok(())
}

fn plot_latency_comparison(results: &BenchmarkResults, path: &str) -> Result<()> {
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let methods = vec![
        ("Vanilla RAG", results.vanilla_rag.avg_latency_ms),
        ("GraphRAG Local", results.graphrag_local.avg_latency_ms),
        ("GraphRAG Global", results.graphrag_global.avg_latency_ms),
    ];

    let max_latency = methods.iter().map(|(_, l)| *l).fold(0.0f64, f64::max);

    let mut chart = ChartBuilder::on(&root)
        .caption("Average Query Latency (ms)", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(0f64..3f64, 0f64..(max_latency * 1.2))?;

    chart.configure_mesh()
        .y_desc("Latency (ms)")
        .draw()?;

    for (i, (method, latency)) in methods.iter().enumerate() {
        chart.draw_series(std::iter::once(Rectangle::new([
            (i as f64 + 0.2, 0.0),
            (i as f64 + 0.8, *latency),
        ], BLUE.filled())))?
        .label(*method)
        .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], BLUE.filled()));
    }

    chart.configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    root.present()?;
    println!("Saved latency plot to {}", path);
    Ok(())
}

fn plot_quality_by_category(results: &BenchmarkResults, path: &str) -> Result<()> {
    let root = BitMapBackend::new(path, (1000, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Quality Score by Question Category", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(80)
        .y_label_area_size(60)
        .build_cartesian_2d(0f64..4f64, 0f64..1.0f64)?;

    chart.configure_mesh()
        .y_desc("Quality Score")
        .x_labels(4)
        .draw()?;

    // Get all categories
    let categories: Vec<String> = results.vanilla_rag.by_category.iter()
        .map(|c| c.category.clone())
        .collect();

    for (cat_idx, category) in categories.iter().enumerate() {
        let x = cat_idx as f64 + 0.5;

        // Vanilla
        if let Some(cat) = results.vanilla_rag.by_category.iter().find(|c| &c.category == category) {
            chart.draw_series(std::iter::once(Circle::new((x - 0.2, cat.avg_quality), 5, RED.filled())))?;
        }

        // Local
        if let Some(cat) = results.graphrag_local.by_category.iter().find(|c| &c.category == category) {
            chart.draw_series(std::iter::once(Circle::new((x, cat.avg_quality), 5, BLUE.filled())))?;
        }

        // Global (only for thematic)
        if let Some(cat) = results.graphrag_global.by_category.iter().find(|c| &c.category == category) {
            chart.draw_series(std::iter::once(Circle::new((x + 0.2, cat.avg_quality), 5, GREEN.filled())))?;
        }
    }

    root.present()?;
    println!("Saved quality plot to {}", path);
    Ok(())
}