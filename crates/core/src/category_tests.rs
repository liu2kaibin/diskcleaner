use crate::category::classify_node;
use crate::rules::Category;
use crate::scanner::Node;

#[test]
fn classify_ai_models() {
    assert_eq!(classify_node(".ollama", true, None), Category::AiModel);
    assert_eq!(classify_node("huggingface", true, None), Category::AiModel);
    assert_eq!(classify_node("deepseek-coder", true, None), Category::AiModel);
}

#[test]
fn classify_package_caches() {
    assert_eq!(classify_node(".npm", true, None), Category::PackageCache);
    assert_eq!(classify_node("npm-cache", true, None), Category::PackageCache);
    assert_eq!(classify_node(".gradle", true, None), Category::PackageCache);
    assert_eq!(classify_node(".cargo", true, None), Category::PackageCache);
    assert_eq!(classify_node("pip", true, None), Category::PackageCache);
}

#[test]
fn classify_sdks() {
    assert_eq!(classify_node("Android", true, None), Category::Sdk);
    assert_eq!(classify_node("AndroidSDK", true, None), Category::Sdk);
    assert_eq!(classify_node("Xcode", true, None), Category::Sdk);
    assert_eq!(classify_node("DerivedData", true, None), Category::Sdk);
}

#[test]
fn classify_documents_and_media() {
    assert_eq!(classify_node("Documents", true, None), Category::DocumentMedia);
    assert_eq!(classify_node("Pictures", true, None), Category::DocumentMedia);
    assert_eq!(classify_node("Music", true, None), Category::DocumentMedia);
    assert_eq!(classify_node("Videos", true, None), Category::DocumentMedia);
    assert_eq!(classify_node("photo.jpg", false, None), Category::DocumentMedia);
    assert_eq!(classify_node("report.pdf", false, None), Category::DocumentMedia);
}

#[test]
fn classify_code_projects_by_markers() {
    // 目录包含 Cargo.toml 或 package.json 等特征文件
    let children = vec![
        Node::file("Cargo.toml", 100),
        Node::dir("src", 500, vec![]),
    ];
    assert_eq!(classify_node("my-app", true, Some(&children)), Category::CodeProject);

    let js_children = vec![
        Node::file("package.json", 100),
    ];
    assert_eq!(classify_node("web-client", true, Some(&js_children)), Category::CodeProject);
}

#[test]
fn classify_dev_tools_and_other() {
    assert_eq!(classify_node(".vscode", true, None), Category::DevToolOther);
    assert_eq!(classify_node(".cursor", true, None), Category::DevToolOther);
    assert_eq!(classify_node(".git", true, None), Category::DevToolOther);
    assert_eq!(classify_node(".idea", true, None), Category::DevToolOther);
}
