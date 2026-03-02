pub(crate) mod css;
pub(crate) mod parsing;
pub(crate) mod types;
pub(crate) mod xpath;

use std::path::Path;

use clap::{Parser, Subcommand};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorCode, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};

use crate::types::{CssSelectorForRegexResponse, EvaluateXPathResult, XPathForRegexResponse};

#[derive(Parser)]
#[command(name = "html-xpath-regex")]
#[command(about = "Provides regex to xpath/CSS selector translation for xml and html documents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "mcp")]
    #[command(about = "Run in MCP mode (stdio transport)")]
    Mcp,
    
    #[command(name = "xpath-for-regex")]
    #[command(about = "Find xpaths for strings matching a regex")]
    XPathForRegex {
        #[arg(name = "file")]
        file: String,
        #[arg(name = "regex")]
        regex: String,
    },
    
    #[command(name = "evaluate-xpath")]
    #[command(about = "Evaluate an XPath expression")]
    EvaluateXPath {
        #[arg(name = "file")]
        file: String,
        #[arg(name = "xpath")]
        xpath: String,
    },
    
    #[command(name = "css-for-regex")]
    #[command(about = "Find CSS selectors for strings matching a regex")]
    CssForRegex {
        #[arg(name = "file")]
        file: String,
        #[arg(name = "regex")]
        regex: String,
    },
    
    #[command(name = "evaluate-css")]
    #[command(about = "Evaluate a CSS selector")]
    EvaluateCss {
        #[arg(name = "file")]
        file: String,
        #[arg(name = "selector")]
        selector: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Mcp => {
            let service = HtmlRegexXpathTool::new().serve(stdio()).await.unwrap();
            service.waiting().await.unwrap();
        }
        Commands::XPathForRegex { file, regex } => {
            run_xpath_for_regex(&file, &regex);
        }
        Commands::EvaluateXPath { file, xpath } => {
            run_evaluate_xpath(&file, &xpath);
        }
        Commands::CssForRegex { file, regex } => {
            run_css_for_regex(&file, &regex);
        }
        Commands::EvaluateCss { file, selector } => {
            run_evaluate_css(&file, &selector);
        }
    }
}

#[derive(Debug, Clone)]
pub struct HtmlRegexXpathTool {
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl HtmlRegexXpathTool {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Provides a list of xpaths for strings matching a regex in an HTML or XML file")]
    pub fn xpath_for_regex(
        &self,
        Parameters(XPathForRegexRequest { file, regex }): Parameters<XPathForRegexRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("File not found: {file}"),
                None,
            ));
        }

        let compiled_regex = regex::Regex::new(&regex).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Invalid regex pattern: {e}"),
                None,
            )
        })?;

        let root = parsing::parse_file(file_path).map_err(|e| {
            ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None)
        })?;

        let matches = xpath::xpath_for_regex(&root, &compiled_regex);

        let response = XPathForRegexResponse {
            file,
            regex,
            matches,
        };

        let json = serde_json::to_string_pretty(&response).map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Evaluates an XPath expression against an HTML or XML file and returns the matching content")]
    pub fn evaluate_xpath(
        &self,
        Parameters(EvaluateXPathRequest { file, xpath }): Parameters<EvaluateXPathRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("File not found: {file}"),
                None,
            ));
        }

        let root = parsing::parse_file(file_path).map_err(|e| {
            ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None)
        })?;

        let results = xpath::evaluate_xpath(&root, &xpath).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Invalid XPath expression: {e}"),
                None,
            )
        })?;

        let response = EvaluateXPathResponse {
            file,
            xpath,
            results,
        };

        let json = serde_json::to_string_pretty(&response).map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Provides a list of CSS selectors for elements containing strings matching a regex in an HTML or XML file")]
    pub fn css_selector_for_regex(
        &self,
        Parameters(CssSelectorForRegexRequest { file, regex }): Parameters<CssSelectorForRegexRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("File not found: {file}"),
                None,
            ));
        }

        let compiled_regex = regex::Regex::new(&regex).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Invalid regex pattern: {e}"),
                None,
            )
        })?;

        let root = parsing::parse_file(file_path).map_err(|e| {
            ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None)
        })?;

        let matches = css::css_selector_for_regex(&root, &compiled_regex);

        let response = CssSelectorForRegexResponse {
            file,
            regex,
            matches,
        };

        let json = serde_json::to_string_pretty(&response).map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Evaluates a CSS selector against an HTML or XML file and returns the matching content")]
    pub fn evaluate_css_selector(
        &self,
        Parameters(EvaluateCssSelectorRequest { file, selector }): Parameters<EvaluateCssSelectorRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("File not found: {file}"),
                None,
            ));
        }

        let root = parsing::parse_file(file_path).map_err(|e| {
            ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None)
        })?;

        let results = css::evaluate_css_selector(&root, &selector).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Invalid CSS selector: {e}"),
                None,
            )
        })?;

        let response = EvaluateCssSelectorResponse {
            file,
            selector,
            results,
        };

        let json = serde_json::to_string_pretty(&response).map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XPathForRegexRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The regex to search, and the results will be translated back to xpaths")]
    pub regex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvaluateXPathRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The XPath expression to evaluate")]
    pub xpath: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvaluateXPathResponse {
    pub file: String,
    pub xpath: String,
    pub results: Vec<EvaluateXPathResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CssSelectorForRegexRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The regex to search, and the results will be translated back to CSS selectors")]
    pub regex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvaluateCssSelectorRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The CSS selector to evaluate")]
    pub selector: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvaluateCssSelectorResponse {
    pub file: String,
    pub selector: String,
    pub results: Vec<EvaluateXPathResult>,
}

fn run_xpath_for_regex(file: &str, regex: &str) {
    let file_path = Path::new(file);
    if !file_path.exists() {
        eprintln!("Error: File not found: {file}");
        std::process::exit(1);
    }

    let compiled_regex = match regex::Regex::new(regex) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Invalid regex pattern: {e}");
            std::process::exit(1);
        }
    };

    let root = match parsing::parse_file(file_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to parse file: {e}");
            std::process::exit(1);
        }
    };

    let matches = xpath::xpath_for_regex(&root, &compiled_regex);

    let response = XPathForRegexResponse {
        file: file.to_string(),
        regex: regex.to_string(),
        matches,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    println!("{json}");
}

fn run_evaluate_xpath(file: &str, xpath: &str) {
    let file_path = Path::new(file);
    if !file_path.exists() {
        eprintln!("Error: File not found: {file}");
        std::process::exit(1);
    }

    let root = match parsing::parse_file(file_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to parse file: {e}");
            std::process::exit(1);
        }
    };

    let results = match xpath::evaluate_xpath(&root, xpath) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Invalid XPath expression: {e}");
            std::process::exit(1);
        }
    };

    let response = EvaluateXPathResponse {
        file: file.to_string(),
        xpath: xpath.to_string(),
        results,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    println!("{json}");
}

fn run_css_for_regex(file: &str, regex: &str) {
    let file_path = Path::new(file);
    if !file_path.exists() {
        eprintln!("Error: File not found: {file}");
        std::process::exit(1);
    }

    let compiled_regex = match regex::Regex::new(regex) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Invalid regex pattern: {e}");
            std::process::exit(1);
        }
    };

    let root = match parsing::parse_file(file_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to parse file: {e}");
            std::process::exit(1);
        }
    };

    let matches = css::css_selector_for_regex(&root, &compiled_regex);

    let response = CssSelectorForRegexResponse {
        file: file.to_string(),
        regex: regex.to_string(),
        matches,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    println!("{json}");
}

fn run_evaluate_css(file: &str, selector: &str) {
    let file_path = Path::new(file);
    if !file_path.exists() {
        eprintln!("Error: File not found: {file}");
        std::process::exit(1);
    }

    let root = match parsing::parse_file(file_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Failed to parse file: {e}");
            std::process::exit(1);
        }
    };

    let results = match css::evaluate_css_selector(&root, selector) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Invalid CSS selector: {e}");
            std::process::exit(1);
        }
    };

    let response = EvaluateCssSelectorResponse {
        file: file.to_string(),
        selector: selector.to_string(),
        results,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    println!("{json}");
}

#[tool_handler]
impl ServerHandler for HtmlRegexXpathTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Provides regex to xpath/CSS selector translation for xml and html documents".to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
