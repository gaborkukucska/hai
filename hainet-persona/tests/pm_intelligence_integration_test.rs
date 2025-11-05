//! Integration tests for PM Intelligence module

use hainet_persona::agents::{
    ProjectComplexity, HistoricalLearner, DecompositionStrategy, ProjectOutcome,
};
use std::time::SystemTime;

#[test]
fn test_complexity_analysis_integration() {
    let complexity = ProjectComplexity::analyze(
        "Build a REST API with authentication, database, and deployment pipeline",
        &vec![
            "Design API endpoints".to_string(),
            "Implement authentication".to_string(),
            "Set up database".to_string(),
            "Write tests".to_string(),
            "Deploy to production".to_string(),
        ],
    );
    
    assert_eq!(complexity.task_count, 5);
    assert!(complexity.score > 0.4, "Complex project should have score > 0.4");
    assert_eq!(complexity.category(), "moderate");
    assert!(complexity.domain_count >= 2, "Should detect multiple domains");
}

#[test]
fn test_learner_integration() {
    let mut learner = HistoricalLearner::new();
    
    // Simulate a few successful sequential projects
    for i in 0..3 {
        learner.record_outcome(ProjectOutcome {
            project_id: format!("proj_{}", i),
            strategy: DecompositionStrategy::Sequential,
            complexity: ProjectComplexity {
                task_count: 3,
                estimated_size: 300,
                domain_count: 1,
                has_external_deps: false,
                score: 0.25,
            },
            success: true,
            duration_secs: 180,
            revision_count: 0,
            timestamp: SystemTime::now(),
        });
    }
    
    // Simulate a failed parallel project
    learner.record_outcome(ProjectOutcome {
        project_id: "proj_fail".to_string(),
        strategy: DecompositionStrategy::Parallel,
        complexity: ProjectComplexity {
            task_count: 3,
            estimated_size: 320,
            domain_count: 1,
            has_external_deps: false,
            score: 0.28,
        },
        success: false,
        duration_secs: 300,
        revision_count: 3,
        timestamp: SystemTime::now(),
    });
    
    // Query for similar complexity
    let test_complexity = ProjectComplexity {
        task_count: 3,
        estimated_size: 310,
        domain_count: 1,
        has_external_deps: false,
        score: 0.26,
    };
    
    let recommended = learner.recommend_strategy(&test_complexity);
    
    // Should recommend Sequential based on historical success
    assert_eq!(recommended, DecompositionStrategy::Sequential);
    assert_eq!(learner.outcome_count(), 4);
}

#[test]
fn test_strategy_learning_convergence() {
    let mut learner = HistoricalLearner::new();
    
    // Record 10 successful hybrid projects
    for i in 0..10 {
        learner.record_outcome(ProjectOutcome {
            project_id: format!("proj_{}", i),
            strategy: DecompositionStrategy::Hybrid,
            complexity: ProjectComplexity {
                task_count: 6,
                estimated_size: 800,
                domain_count: 3,
                has_external_deps: true,
                score: 0.65,
            },
            success: true,
            duration_secs: 600,
            revision_count: 1,
            timestamp: SystemTime::now(),
        });
    }
    
    // Query for similar complex project
    let complex = ProjectComplexity {
        task_count: 7,
        estimated_size: 850,
        domain_count: 3,
        has_external_deps: true,
        score: 0.68,
    };
    
    let recommended = learner.recommend_strategy(&complex);
    assert_eq!(recommended, DecompositionStrategy::Hybrid);
    
    let success_rate = learner.strategy_success_rate(DecompositionStrategy::Hybrid);
    assert_eq!(success_rate, 1.0, "All hybrid projects succeeded");
}

#[test]
fn test_learner_capacity_limit() {
    let mut learner = HistoricalLearner::with_capacity(5);
    
    // Add 10 outcomes (should only keep last 5)
    for i in 0..10 {
        learner.record_outcome(ProjectOutcome {
            project_id: format!("proj_{}", i),
            strategy: DecompositionStrategy::Sequential,
            complexity: ProjectComplexity {
                task_count: 2,
                estimated_size: 200,
                domain_count: 1,
                has_external_deps: false,
                score: 0.2,
            },
            success: true,
            duration_secs: 120,
            revision_count: 0,
            timestamp: SystemTime::now(),
        });
    }
    
    assert_eq!(learner.outcome_count(), 5, "Should trim to max capacity");
}

#[test]
fn test_mixed_strategy_performance() {
    let mut learner = HistoricalLearner::new();
    
    // Sequential: 2 success, 1 failure
    for i in 0..2 {
        learner.record_outcome(ProjectOutcome {
            project_id: format!("seq_{}", i),
            strategy: DecompositionStrategy::Sequential,
            complexity: ProjectComplexity {
                task_count: 4,
                estimated_size: 400,
                domain_count: 2,
                has_external_deps: false,
                score: 0.35,
            },
            success: true,
            duration_secs: 240,
            revision_count: 0,
            timestamp: SystemTime::now(),
        });
    }
    
    learner.record_outcome(ProjectOutcome {
        project_id: "seq_fail".to_string(),
        strategy: DecompositionStrategy::Sequential,
        complexity: ProjectComplexity {
            task_count: 4,
            estimated_size: 400,
            domain_count: 2,
            has_external_deps: false,
            score: 0.35,
        },
        success: false,
        duration_secs: 400,
        revision_count: 2,
        timestamp: SystemTime::now(),
    });
    
    // Parallel: 3 success
    for i in 0..3 {
        learner.record_outcome(ProjectOutcome {
            project_id: format!("par_{}", i),
            strategy: DecompositionStrategy::Parallel,
            complexity: ProjectComplexity {
                task_count: 4,
                estimated_size: 420,
                domain_count: 2,
                has_external_deps: false,
                score: 0.38,
            },
            success: true,
            duration_secs: 200,
            revision_count: 0,
            timestamp: SystemTime::now(),
        });
    }
    
    // Query for similar project
    let test = ProjectComplexity {
        task_count: 4,
        estimated_size: 410,
        domain_count: 2,
        has_external_deps: false,
        score: 0.36,
    };
    
    let recommended = learner.recommend_strategy(&test);
    
    // Should recommend Parallel (3/3 = 100%) over Sequential (2/3 = 66%)
    assert_eq!(recommended, DecompositionStrategy::Parallel);
}
