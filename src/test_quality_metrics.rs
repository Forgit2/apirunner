use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;


use crate::error::TestCaseError;
use crate::test_case_manager::TestCase;
use crate::test_case_linter::LintingReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestQualityMetrics {
    pub coverage_metrics: CoverageMetrics,
    pub maintenance_metrics: MaintenanceMetrics,
    pub collaboration_metrics: CollaborationMetrics,
    pub quality_score: f64,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMetrics {
    pub total_endpoints: usize,
    pub covered_endpoints: usize,
    pub coverage_percentage: f64,
    pub http_methods_coverage: HashMap<String, MethodCoverage>,
    pub status_codes_coverage: HashMap<u16, usize>,
    pub endpoint_coverage_details: Vec<EndpointCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodCoverage {
    pub total_endpoints: usize,
    pub covered_endpoints: usize,
    pub coverage_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointCoverage {
    pub endpoint: String,
    pub method: String,
    pub is_covered: bool,
    pub test_cases: Vec<String>,
    pub missing_scenarios: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceMetrics {
    pub test_age_distribution: HashMap<String, usize>, // "0-30 days", "31-90 days", etc.
    pub outdated_tests: Vec<OutdatedTest>,
    pub duplicate_tests: Vec<DuplicateTestGroup>,
    pub maintenance_recommendations: Vec<MaintenanceRecommendation>,
    pub technical_debt_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutdatedTest {
    pub test_id: String,
    pub test_name: String,
    pub last_updated: DateTime<Utc>,
    pub days_since_update: i64,
    pub reason: OutdatedReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutdatedReason {
    NotUpdatedRecently,
    FailingConsistently,
    UsingDeprecatedFeatures,
    MissingAssertions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateTestGroup {
    pub similarity_score: f64,
    pub test_cases: Vec<DuplicateTestInfo>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateTestInfo {
    pub test_id: String,
    pub test_name: String,
    pub endpoint: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceRecommendation {
    pub category: MaintenanceCategory,
    pub priority: RecommendationPriority,
    pub title: String,
    pub description: String,
    pub affected_tests: Vec<String>,
    pub estimated_effort: EstimatedEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MaintenanceCategory {
    Cleanup,
    Optimization,
    Standardization,
    Coverage,
    Quality,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecommendationPriority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EstimatedEffort {
    Low,    // < 1 hour
    Medium, // 1-4 hours
    High,   // > 4 hours
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationMetrics {
    pub contributors: Vec<ContributorMetrics>,
    pub review_metrics: ReviewMetrics,
    pub team_coverage_distribution: HashMap<String, f64>,
    pub knowledge_sharing_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributorMetrics {
    pub contributor: String,
    pub tests_created: usize,
    pub tests_modified: usize,
    pub last_contribution: DateTime<Utc>,
    pub coverage_contribution: f64,
    pub quality_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewMetrics {
    pub total_reviews: usize,
    pub average_review_time_hours: f64,
    pub review_participation_rate: f64,
    pub common_review_issues: Vec<ReviewIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub issue_type: String,
    pub frequency: usize,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetricsConfig {
    pub coverage_threshold: f64,
    pub maintenance_threshold_days: i64,
    pub duplicate_similarity_threshold: f64,
    pub quality_score_weights: QualityScoreWeights,
    pub excluded_endpoints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScoreWeights {
    pub coverage_weight: f64,
    pub maintenance_weight: f64,
    pub collaboration_weight: f64,
    pub linting_weight: f64,
}

impl Default for QualityMetricsConfig {
    fn default() -> Self {
        Self {
            coverage_threshold: 80.0,
            maintenance_threshold_days: 90,
            duplicate_similarity_threshold: 0.8,
            quality_score_weights: QualityScoreWeights {
                coverage_weight: 0.3,
                maintenance_weight: 0.3,
                collaboration_weight: 0.2,
                linting_weight: 0.2,
            },
            excluded_endpoints: vec![],
        }
    }
}

pub struct TestQualityAnalyzer {
    config: QualityMetricsConfig,
}

impl TestQualityAnalyzer {
    pub fn new(config: QualityMetricsConfig) -> Self {
        Self { config }
    }

    pub async fn analyze_test_quality(
        &self,
        test_cases: &[TestCase],
        linting_report: Option<&LintingReport>,
    ) -> Result<TestQualityMetrics, TestCaseError> {
        let coverage_metrics = self.analyze_coverage(test_cases).await?;
        let maintenance_metrics = self.analyze_maintenance(test_cases).await?;
        let collaboration_metrics = self.analyze_collaboration(test_cases).await?;
        
        let quality_score = self.calculate_overall_quality_score(
            &coverage_metrics,
            &maintenance_metrics,
            &collaboration_metrics,
            linting_report,
        );

        Ok(TestQualityMetrics {
            coverage_metrics,
            maintenance_metrics,
            collaboration_metrics,
            quality_score,
            generated_at: Utc::now(),
        })
    }

    async fn analyze_coverage(&self, test_cases: &[TestCase]) -> Result<CoverageMetrics, TestCaseError> {
        let mut endpoint_map: HashMap<String, Vec<&TestCase>> = HashMap::new();
        let mut method_stats: HashMap<String, (usize, usize)> = HashMap::new(); // (total, covered)
        let mut status_codes: HashMap<u16, usize> = HashMap::new();

        // Group test cases by endpoint
        for test_case in test_cases {
            let endpoint_key = self.normalize_endpoint(&test_case.request.url);
            endpoint_map.entry(endpoint_key).or_insert_with(Vec::new).push(test_case);
            
            // Track method usage
            let method = test_case.request.method.to_uppercase();
            method_stats.entry(method).or_insert((0, 0)).1 += 1;
            
            // Track expected status codes from assertions
            for assertion in &test_case.assertions {
                if assertion.assertion_type == "status_code" {
                    if let Some(status) = assertion.expected.as_u64() {
                        *status_codes.entry(status as u16).or_insert(0) += 1;
                    }
                }
            }
        }

        // Calculate coverage metrics
        let total_endpoints = endpoint_map.len();
        let covered_endpoints = endpoint_map.values().filter(|tests| !tests.is_empty()).count();
        let coverage_percentage = if total_endpoints > 0 {
            (covered_endpoints as f64 / total_endpoints as f64) * 100.0
        } else {
            0.0
        };

        // Calculate method coverage
        let http_methods_coverage = method_stats
            .into_iter()
            .map(|(method, (total, covered))| {
                let coverage_percentage = if total > 0 {
                    (covered as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                (
                    method,
                    MethodCoverage {
                        total_endpoints: total,
                        covered_endpoints: covered,
                        coverage_percentage,
                    },
                )
            })
            .collect();

        // Generate endpoint coverage details
        let endpoint_coverage_details = endpoint_map
            .into_iter()
            .map(|(endpoint, tests)| {
                let is_covered = !tests.is_empty();
                let test_cases = tests.iter().map(|t| t.name.clone()).collect();
                let missing_scenarios = self.identify_missing_scenarios(&endpoint, &tests);
                
                EndpointCoverage {
                    endpoint: endpoint.clone(),
                    method: tests.first().map(|t| t.request.method.clone()).unwrap_or_default(),
                    is_covered,
                    test_cases,
                    missing_scenarios,
                }
            })
            .collect();

        Ok(CoverageMetrics {
            total_endpoints,
            covered_endpoints,
            coverage_percentage,
            http_methods_coverage,
            status_codes_coverage: status_codes,
            endpoint_coverage_details,
        })
    }

    async fn analyze_maintenance(&self, test_cases: &[TestCase]) -> Result<MaintenanceMetrics, TestCaseError> {
        let now = Utc::now();
        let mut age_distribution: HashMap<String, usize> = HashMap::new();
        let mut outdated_tests = Vec::new();
        let mut recommendations = Vec::new();

        // Analyze test age and identify outdated tests
        for test_case in test_cases {
            let duration = now.signed_duration_since(test_case.updated_at);
            let days_since_update = duration.num_days();
            
            // Categorize by age
            let age_category = match days_since_update {
                0..=30 => "0-30 days",
                31..=90 => "31-90 days",
                91..=180 => "91-180 days",
                181..=365 => "181-365 days",
                _ => "Over 1 year",
            };
            *age_distribution.entry(age_category.to_string()).or_insert(0) += 1;

            // Identify outdated tests
            if days_since_update > self.config.maintenance_threshold_days {
                let reason = self.determine_outdated_reason(test_case, days_since_update);
                outdated_tests.push(OutdatedTest {
                    test_id: test_case.id.clone(),
                    test_name: test_case.name.clone(),
                    last_updated: test_case.updated_at,
                    days_since_update,
                    reason,
                });
            }
        }

        // Find duplicate tests
        let duplicate_tests = self.find_duplicate_tests(test_cases);

        // Generate maintenance recommendations
        recommendations.extend(self.generate_maintenance_recommendations(test_cases, &outdated_tests, &duplicate_tests));

        let technical_debt_score = self.calculate_technical_debt_score(test_cases, &outdated_tests, &duplicate_tests);

        Ok(MaintenanceMetrics {
            test_age_distribution: age_distribution,
            outdated_tests,
            duplicate_tests,
            maintenance_recommendations: recommendations,
            technical_debt_score,
        })
    }

    async fn analyze_collaboration(&self, test_cases: &[TestCase]) -> Result<CollaborationMetrics, TestCaseError> {
        let mut contributor_stats: HashMap<String, (usize, usize, DateTime<Utc>)> = HashMap::new(); // (created, modified, last_contribution)
        let mut team_coverage: HashMap<String, usize> = HashMap::new();

        // Analyze contributor metrics
        for test_case in test_cases {
            // For demo purposes, use a default contributor or extract from test case ID
            let contributor = format!("user_{}", test_case.id.chars().take(3).collect::<String>());
            let stats = contributor_stats.entry(contributor.clone()).or_insert((0, 0, test_case.created_at));
            
            stats.0 += 1; // tests created
            if test_case.created_at != test_case.updated_at {
                stats.1 += 1; // tests modified
            }
            if test_case.updated_at > stats.2 {
                stats.2 = test_case.updated_at; // last contribution
            }
            
            *team_coverage.entry(contributor.clone()).or_insert(0) += 1;
        }

        let total_tests = test_cases.len() as f64;
        let team_coverage_distribution: HashMap<String, f64> = team_coverage
            .into_iter()
            .map(|(contributor, count)| (contributor, (count as f64 / total_tests) * 100.0))
            .collect();

        let contributors: Vec<ContributorMetrics> = contributor_stats
            .into_iter()
            .map(|(contributor, (created, modified, last_contribution))| {
                let coverage_contribution = team_coverage_distribution
                    .get(&contributor)
                    .copied()
                    .unwrap_or(0.0);
                
                ContributorMetrics {
                    contributor,
                    tests_created: created,
                    tests_modified: modified,
                    last_contribution,
                    coverage_contribution,
                    quality_score: 85.0, // Placeholder - could be calculated based on linting results
                }
            })
            .collect();

        // Mock review metrics (in a real implementation, this would come from version control or review systems)
        let review_metrics = ReviewMetrics {
            total_reviews: test_cases.len() / 2, // Assume 50% of tests were reviewed
            average_review_time_hours: 2.5,
            review_participation_rate: 75.0,
            common_review_issues: vec![
                ReviewIssue {
                    issue_type: "Missing assertions".to_string(),
                    frequency: 15,
                    description: "Tests lack sufficient assertions to validate behavior".to_string(),
                },
                ReviewIssue {
                    issue_type: "Hardcoded values".to_string(),
                    frequency: 8,
                    description: "Tests contain hardcoded values that should be parameterized".to_string(),
                },
            ],
        };

        let knowledge_sharing_score = self.calculate_knowledge_sharing_score(&contributors);

        Ok(CollaborationMetrics {
            contributors,
            review_metrics,
            team_coverage_distribution,
            knowledge_sharing_score,
        })
    }

    fn calculate_overall_quality_score(
        &self,
        coverage: &CoverageMetrics,
        maintenance: &MaintenanceMetrics,
        collaboration: &CollaborationMetrics,
        linting_report: Option<&LintingReport>,
    ) -> f64 {
        let weights = &self.config.quality_score_weights;
        
        let coverage_score = coverage.coverage_percentage;
        let maintenance_score = 100.0 - maintenance.technical_debt_score;
        let collaboration_score = collaboration.knowledge_sharing_score;
        let linting_score = linting_report
            .map(|report| report.average_score)
            .unwrap_or(100.0);

        (coverage_score * weights.coverage_weight
            + maintenance_score * weights.maintenance_weight
            + collaboration_score * weights.collaboration_weight
            + linting_score * weights.linting_weight)
            .min(100.0)
            .max(0.0)
    }

    fn normalize_endpoint(&self, url: &str) -> String {
        // Extract the path part of the URL and normalize parameters
        if let Some(path_start) = url.find("://") {
            if let Some(path) = url[path_start + 3..].find('/') {
                let full_path = &url[path_start + 3 + path..];
                if let Some(query_start) = full_path.find('?') {
                    full_path[..query_start].to_string()
                } else {
                    full_path.to_string()
                }
            } else {
                "/".to_string()
            }
        } else {
            url.to_string()
        }
    }

    fn identify_missing_scenarios(&self, _endpoint: &str, _tests: &[&TestCase]) -> Vec<String> {
        // Placeholder implementation - could analyze common scenarios like:
        // - Different HTTP methods
        // - Error cases (4xx, 5xx status codes)
        // - Edge cases (empty body, invalid parameters)
        vec![
            "Error handling (4xx responses)".to_string(),
            "Server error handling (5xx responses)".to_string(),
            "Input validation scenarios".to_string(),
        ]
    }

    fn determine_outdated_reason(&self, test_case: &TestCase, days_since_update: i64) -> OutdatedReason {
        // Simple heuristics to determine why a test might be outdated
        if days_since_update > 365 {
            OutdatedReason::NotUpdatedRecently
        } else if test_case.assertions.is_empty() {
            OutdatedReason::MissingAssertions
        } else {
            OutdatedReason::NotUpdatedRecently
        }
    }

    fn find_duplicate_tests(&self, test_cases: &[TestCase]) -> Vec<DuplicateTestGroup> {
        let mut duplicates: Vec<DuplicateTestGroup> = Vec::new();
        
        for (i, test1) in test_cases.iter().enumerate() {
            for test2 in test_cases.iter().skip(i + 1) {
                let similarity = self.calculate_test_similarity(test1, test2);
                if similarity >= self.config.duplicate_similarity_threshold {
                    // Check if this pair is already in a group
                    let mut found_group = false;
                    for group in &mut duplicates {
                        if group.test_cases.iter().any(|t| t.test_id == test1.id || t.test_id == test2.id) {
                            // Add to existing group if not already present
                            if !group.test_cases.iter().any(|t| t.test_id == test1.id) {
                                group.test_cases.push(DuplicateTestInfo {
                                    test_id: test1.id.clone(),
                                    test_name: test1.name.clone(),
                                    endpoint: test1.request.url.clone(),
                                    method: test1.request.method.clone(),
                                });
                            }
                            if !group.test_cases.iter().any(|t| t.test_id == test2.id) {
                                group.test_cases.push(DuplicateTestInfo {
                                    test_id: test2.id.clone(),
                                    test_name: test2.name.clone(),
                                    endpoint: test2.request.url.clone(),
                                    method: test2.request.method.clone(),
                                });
                            }
                            found_group = true;
                            break;
                        }
                    }
                    
                    if !found_group {
                        duplicates.push(DuplicateTestGroup {
                            similarity_score: similarity,
                            test_cases: vec![
                                DuplicateTestInfo {
                                    test_id: test1.id.clone(),
                                    test_name: test1.name.clone(),
                                    endpoint: test1.request.url.clone(),
                                    method: test1.request.method.clone(),
                                },
                                DuplicateTestInfo {
                                    test_id: test2.id.clone(),
                                    test_name: test2.name.clone(),
                                    endpoint: test2.request.url.clone(),
                                    method: test2.request.method.clone(),
                                },
                            ],
                            recommendation: "Consider consolidating these similar tests or ensuring they test different scenarios".to_string(),
                        });
                    }
                }
            }
        }
        
        duplicates
    }

    fn calculate_test_similarity(&self, test1: &TestCase, test2: &TestCase) -> f64 {
        let mut similarity_factors = Vec::new();
        
        // URL similarity
        let url_similarity = if test1.request.url == test2.request.url { 1.0 } else { 0.0 };
        similarity_factors.push(url_similarity * 0.4);
        
        // Method similarity
        let method_similarity = if test1.request.method == test2.request.method { 1.0 } else { 0.0 };
        similarity_factors.push(method_similarity * 0.3);
        
        // Headers similarity
        let headers_similarity = self.calculate_headers_similarity(&test1.request.headers, &test2.request.headers);
        similarity_factors.push(headers_similarity * 0.2);
        
        // Body similarity
        let body_similarity = self.calculate_body_similarity(&test1.request.body, &test2.request.body);
        similarity_factors.push(body_similarity * 0.1);
        
        similarity_factors.iter().sum()
    }

    fn calculate_headers_similarity(&self, headers1: &HashMap<String, String>, headers2: &HashMap<String, String>) -> f64 {
        if headers1.is_empty() && headers2.is_empty() {
            return 1.0;
        }
        
        let all_keys: std::collections::HashSet<_> = headers1.keys().chain(headers2.keys()).collect();
        let matching_keys = all_keys.iter().filter(|key| {
            headers1.get(key.as_str()) == headers2.get(key.as_str())
        }).count();
        
        matching_keys as f64 / all_keys.len() as f64
    }

    fn calculate_body_similarity(&self, body1: &Option<String>, body2: &Option<String>) -> f64 {
        match (body1, body2) {
            (None, None) => 1.0,
            (Some(b1), Some(b2)) => if b1 == b2 { 1.0 } else { 0.5 },
            _ => 0.0,
        }
    }

    fn generate_maintenance_recommendations(
        &self,
        test_cases: &[TestCase],
        outdated_tests: &[OutdatedTest],
        duplicate_tests: &[DuplicateTestGroup],
    ) -> Vec<MaintenanceRecommendation> {
        let mut recommendations = Vec::new();

        // Outdated tests recommendation
        if !outdated_tests.is_empty() {
            recommendations.push(MaintenanceRecommendation {
                category: MaintenanceCategory::Cleanup,
                priority: RecommendationPriority::High,
                title: "Update outdated test cases".to_string(),
                description: format!("Review and update {} test cases that haven't been modified recently", outdated_tests.len()),
                affected_tests: outdated_tests.iter().map(|t| t.test_id.clone()).collect(),
                estimated_effort: if outdated_tests.len() > 10 { EstimatedEffort::High } else { EstimatedEffort::Medium },
            });
        }

        // Duplicate tests recommendation
        if !duplicate_tests.is_empty() {
            let total_duplicates: usize = duplicate_tests.iter().map(|g| g.test_cases.len()).sum();
            recommendations.push(MaintenanceRecommendation {
                category: MaintenanceCategory::Optimization,
                priority: RecommendationPriority::Medium,
                title: "Consolidate duplicate test cases".to_string(),
                description: format!("Review {} groups of similar test cases for potential consolidation", duplicate_tests.len()),
                affected_tests: duplicate_tests.iter()
                    .flat_map(|g| g.test_cases.iter().map(|t| t.test_id.clone()))
                    .collect(),
                estimated_effort: if total_duplicates > 20 { EstimatedEffort::High } else { EstimatedEffort::Medium },
            });
        }

        // Coverage improvement recommendation
        let tests_without_assertions = test_cases.iter()
            .filter(|t| t.assertions.is_empty())
            .count();
        
        if tests_without_assertions > 0 {
            recommendations.push(MaintenanceRecommendation {
                category: MaintenanceCategory::Quality,
                priority: RecommendationPriority::High,
                title: "Add missing assertions".to_string(),
                description: format!("Add assertions to {} test cases that lack proper validation", tests_without_assertions),
                affected_tests: test_cases.iter()
                    .filter(|t| t.assertions.is_empty())
                    .map(|t| t.id.clone())
                    .collect(),
                estimated_effort: EstimatedEffort::Medium,
            });
        }

        recommendations
    }

    fn calculate_technical_debt_score(
        &self,
        test_cases: &[TestCase],
        outdated_tests: &[OutdatedTest],
        duplicate_tests: &[DuplicateTestGroup],
    ) -> f64 {
        let total_tests = test_cases.len() as f64;
        if total_tests == 0.0 {
            return 0.0;
        }

        let outdated_ratio = outdated_tests.len() as f64 / total_tests;
        let duplicate_ratio = duplicate_tests.iter().map(|g| g.test_cases.len()).sum::<usize>() as f64 / total_tests;
        let missing_assertions_ratio = test_cases.iter()
            .filter(|t| t.assertions.is_empty())
            .count() as f64 / total_tests;

        // Technical debt score (0-100, where 100 is maximum debt)
        ((outdated_ratio * 40.0) + (duplicate_ratio * 30.0) + (missing_assertions_ratio * 30.0))
            .min(100.0)
    }

    fn calculate_knowledge_sharing_score(&self, contributors: &[ContributorMetrics]) -> f64 {
        if contributors.is_empty() {
            return 0.0;
        }

        // Calculate distribution of contributions
        let total_contributions: usize = contributors.iter().map(|c| c.tests_created + c.tests_modified).sum();
        if total_contributions == 0 {
            return 0.0;
        }

        // Calculate Gini coefficient to measure inequality in contributions
        let mut contributions: Vec<f64> = contributors
            .iter()
            .map(|c| (c.tests_created + c.tests_modified) as f64)
            .collect();
        contributions.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = contributions.len() as f64;
        let sum_contributions: f64 = contributions.iter().sum();
        
        if sum_contributions == 0.0 {
            return 0.0;
        }

        let mut gini_sum = 0.0;
        for (i, contribution) in contributions.iter().enumerate() {
            gini_sum += (2.0 * (i as f64 + 1.0) - n - 1.0) * contribution;
        }

        let gini_coefficient = gini_sum / (n * sum_contributions);
        
        // Convert Gini coefficient to knowledge sharing score (lower Gini = better sharing)
        (1.0 - gini_coefficient.abs()) * 100.0
    }
}

pub struct QualityMetricsReporter;

impl QualityMetricsReporter {
    pub fn generate_report(&self, metrics: &TestQualityMetrics) -> String {
        let mut report = String::new();
        
        report.push_str("# Test Quality Metrics Report\n\n");
        report.push_str(&format!("**Generated:** {}\n", metrics.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        report.push_str(&format!("**Overall Quality Score:** {:.1}/100\n\n", metrics.quality_score));
        
        // Coverage section
        report.push_str("## Coverage Metrics\n\n");
        report.push_str(&format!("- **Total Endpoints:** {}\n", metrics.coverage_metrics.total_endpoints));
        report.push_str(&format!("- **Covered Endpoints:** {}\n", metrics.coverage_metrics.covered_endpoints));
        report.push_str(&format!("- **Coverage Percentage:** {:.1}%\n\n", metrics.coverage_metrics.coverage_percentage));
        
        // Maintenance section
        report.push_str("## Maintenance Metrics\n\n");
        report.push_str(&format!("- **Technical Debt Score:** {:.1}/100\n", metrics.maintenance_metrics.technical_debt_score));
        report.push_str(&format!("- **Outdated Tests:** {}\n", metrics.maintenance_metrics.outdated_tests.len()));
        report.push_str(&format!("- **Duplicate Test Groups:** {}\n\n", metrics.maintenance_metrics.duplicate_tests.len()));
        
        // Collaboration section
        report.push_str("## Collaboration Metrics\n\n");
        report.push_str(&format!("- **Contributors:** {}\n", metrics.collaboration_metrics.contributors.len()));
        report.push_str(&format!("- **Knowledge Sharing Score:** {:.1}/100\n", metrics.collaboration_metrics.knowledge_sharing_score));
        report.push_str(&format!("- **Review Participation Rate:** {:.1}%\n\n", metrics.collaboration_metrics.review_metrics.review_participation_rate));
        
        // Recommendations
        if !metrics.maintenance_metrics.maintenance_recommendations.is_empty() {
            report.push_str("## Recommendations\n\n");
            for (i, rec) in metrics.maintenance_metrics.maintenance_recommendations.iter().enumerate() {
                report.push_str(&format!("{}. **{}** (Priority: {:?})\n", i + 1, rec.title, rec.priority));
                report.push_str(&format!("   {}\n", rec.description));
                report.push_str(&format!("   Affected tests: {}\n\n", rec.affected_tests.len()));
            }
        }
        
        report
    }
}