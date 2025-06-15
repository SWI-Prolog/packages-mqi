use swipl_rs::{PrologServer, ServerConfig, QueryResult, PrologTerm};

/// This example demonstrates using SWI-Prolog from Rust to solve a practical problem:
/// Finding optimal routes in a transportation network using Prolog's powerful
/// backtracking and constraint solving capabilities.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚂 Railway Route Planner - Powered by SWI-Prolog and Rust\n");
    println!("This demo shows how Rust can leverage Prolog's logic programming");
    println!("to solve complex route-finding problems with constraints.\n");

    // Start the Prolog server
    let config = ServerConfig::default();
    let mut server = PrologServer::new(config)?;
    server.start()?;
    
    let mut session = server.connect()?;
    
    // Build a railway network knowledge base
    println!("📍 Building railway network knowledge base...\n");
    
    // Define direct train connections with distances (km) and travel times (minutes)
    let connections = vec![
        ("london", "paris", 344, 140),        // Eurostar
        ("paris", "brussels", 300, 85),       // Thalys
        ("brussels", "amsterdam", 175, 110),   // IC
        ("amsterdam", "berlin", 650, 380),     // ICE
        ("paris", "berlin", 1050, 510),       // Direct TGV/ICE
        ("paris", "lyon", 465, 120),          // TGV
        ("lyon", "geneva", 150, 115),         // TER
        ("geneva", "zurich", 280, 180),       // IC
        ("zurich", "munich", 310, 240),       // EC
        ("munich", "berlin", 585, 360),       // ICE
        ("brussels", "cologne", 185, 120),    // Thalys
        ("cologne", "frankfurt", 180, 85),    // ICE
        ("frankfurt", "berlin", 550, 270),    // ICE
    ];
    
    // Assert connections into Prolog
    for (from, to, distance, time) in &connections {
        let query = format!(
            "assert(connection('{}', '{}', {}, {}))",
            from, to, distance, time
        );
        session.query(&query, None)?;
        
        // Also assert reverse direction
        let reverse_query = format!(
            "assert(connection('{}', '{}', {}, {}))",
            to, from, distance, time
        );
        session.query(&reverse_query, None)?;
    }
    
    // Define Prolog rules for route finding
    println!("🧠 Defining intelligent route-finding rules...\n");
    
    // Rule: direct route
    session.query(
        "assert((route(Start, End, [Start, End], Distance, Time) :- 
            connection(Start, End, Distance, Time)))",
        None
    )?;
    
    // Rule: indirect route with multiple stops (use tabling for efficiency)
    session.query(
        "assert((route(Start, End, Path, TotalDist, TotalTime) :- 
            route_helper(Start, End, [Start], Path, TotalDist, TotalTime)))",
        None
    )?;
    
    // Helper rule with visited list to avoid cycles
    session.query(
        "assert((route_helper(End, End, Visited, Path, 0, 0) :- 
            reverse(Visited, Path)))",
        None
    )?;
    
    session.query(
        "assert((route_helper(Start, End, Visited, Path, TotalDist, TotalTime) :- 
            connection(Start, Next, Dist1, Time1),
            \\+ member(Next, Visited),
            length(Visited, L), L < 5,  % Max 5 stops
            route_helper(Next, End, [Next|Visited], Path, Dist2, Time2),
            TotalDist is Dist1 + Dist2,
            TotalTime is Time1 + Time2))",
        None
    )?;
    
    // Rule: find shortest route by distance
    session.query(
        "assert((shortest_route(Start, End, Route, Distance, Time) :-
            findall(
                [D, T, R],
                route(Start, End, R, D, T),
                Routes
            ),
            sort(Routes, [[Distance, Time, Route]|_])))",
        None
    )?;
    
    // Rule: find fastest route by time
    session.query(
        "assert((fastest_route(Start, End, Route, Distance, Time) :-
            findall(
                [T, D, R],
                route(Start, End, R, D, T),
                Routes
            ),
            sort(Routes, [[Time, Distance, Route]|_])))",
        None
    )?;
    
    // Now let's use our Prolog-powered route planner!
    println!("🔍 Finding routes from London to Berlin...\n");
    
    // Find some possible routes (limited to avoid overwhelming output)
    match session.query("limit(10, route(london, berlin, Path, Distance, Time))", None)? {
        QueryResult::Solutions(solutions) => {
            println!("Found {} possible routes:\n", solutions.len());
            
            for (i, solution) in solutions.iter().enumerate() {
                if let (Some(path), Some(distance), Some(time)) = 
                    (solution.get("Path"), solution.get("Distance"), solution.get("Time")) {
                    
                    println!("Route {}:", i + 1);
                    print_route(path);
                    if let (PrologTerm::Integer(d), PrologTerm::Integer(t)) = (distance, time) {
                        println!("  📏 Distance: {} km", d);
                        println!("  ⏱️  Time: {} minutes ({:.1} hours)\n", t, *t as f64 / 60.0);
                    }
                }
            }
        }
        _ => println!("No routes found"),
    }
    
    // Find the shortest route by distance
    println!("🎯 Finding the SHORTEST route (by distance)...\n");
    
    match session.query("shortest_route(london, berlin, Path, Distance, Time)", None)? {
        QueryResult::Solutions(solutions) => {
            if let Some(solution) = solutions.first() {
                if let (Some(path), Some(distance), Some(time)) = 
                    (solution.get("Path"), solution.get("Distance"), solution.get("Time")) {
                    
                    print_route(path);
                    if let (PrologTerm::Integer(d), PrologTerm::Integer(t)) = (distance, time) {
                        println!("  📏 Total distance: {} km", d);
                        println!("  ⏱️  Total time: {} minutes ({:.1} hours)\n", t, *t as f64 / 60.0);
                    }
                }
            }
        }
        _ => println!("No route found"),
    }
    
    // Find the fastest route by time
    println!("⚡ Finding the FASTEST route (by time)...\n");
    
    match session.query("fastest_route(london, berlin, Path, Distance, Time)", None)? {
        QueryResult::Solutions(solutions) => {
            if let Some(solution) = solutions.first() {
                if let (Some(path), Some(distance), Some(time)) = 
                    (solution.get("Path"), solution.get("Distance"), solution.get("Time")) {
                    
                    print_route(path);
                    if let (PrologTerm::Integer(d), PrologTerm::Integer(t)) = (distance, time) {
                        println!("  📏 Total distance: {} km", d);
                        println!("  ⏱️  Total time: {} minutes ({:.1} hours)\n", t, *t as f64 / 60.0);
                    }
                }
            }
        }
        _ => println!("No route found"),
    }
    
    // Demonstrate constraint-based search
    println!("🔧 Finding routes with constraints...\n");
    println!("Routes under 1000 km that take less than 8 hours:\n");
    
    match session.query(
        "route(london, berlin, Path, Distance, Time), Distance < 1000, Time < 480",
        None
    )? {
        QueryResult::Solutions(solutions) => {
            for solution in solutions {
                if let (Some(path), Some(distance), Some(time)) = 
                    (solution.get("Path"), solution.get("Distance"), solution.get("Time")) {
                    
                    print_route(path);
                    if let (PrologTerm::Integer(d), PrologTerm::Integer(t)) = (distance, time) {
                        println!("  📏 Distance: {} km, ⏱️  Time: {:.1} hours\n", d, *t as f64 / 60.0);
                    }
                }
            }
        }
        _ => println!("No routes matching constraints"),
    }
    
    // Clean up
    session.close()?;
    server.stop(false)?;
    
    println!("\n✨ This demo showcases how Rust + Prolog can solve complex problems:");
    println!("   - Prolog handles the logic and search algorithms");
    println!("   - Rust provides type safety, performance, and system integration");
    println!("   - Together they create powerful, maintainable applications!");
    
    Ok(())
}

/// Helper function to pretty-print a route
fn print_route(path: &PrologTerm) {
    if let PrologTerm::List(cities) = path {
        print!("  🚂 Route: ");
        for (i, city) in cities.iter().enumerate() {
            if let PrologTerm::Atom(name) = city {
                print!("{}", name);
                if i < cities.len() - 1 {
                    print!(" → ");
                }
            }
        }
        println!();
    }
}