//! 语义类目判定逻辑（Semantic Category）。
//! 依据目录/文件名称与结构启发式识别开发者资产类型，驱动 treemap 着色与图例。

use crate::rules::Category;
use crate::scanner::Node;

/// 依据节点名称、类型与子项内容判定语义类目。
pub fn classify_node(name: &str, is_dir: bool, children: Option<&[Node]>) -> Category {
    let lower = name.to_lowercase();

    // 1. 若为目录且有子项信息，优先探测代码项目特征文件
    if is_dir {
        if let Some(kids) = children {
            for k in kids {
                let kname = k.name();
                if kname == "Cargo.toml"
                    || kname == "package.json"
                    || kname == "go.mod"
                    || kname == "pom.xml"
                    || kname == "build.gradle"
                    || kname == "CMakeLists.txt"
                    || kname == ".git"
                {
                    return Category::CodeProject;
                }
            }
        }
    }

    // 2. AI 模型启发式
    if lower == ".ollama"
        || lower == "ollama"
        || lower.contains("huggingface")
        || lower.contains("deepseek")
        || lower.contains("llama")
        || lower.contains("mistral")
        || lower.contains("qwen")
    {
        return Category::AiModel;
    }

    // 3. 包管理器缓存
    if lower == ".npm"
        || lower == "npm-cache"
        || lower == ".gradle"
        || lower == ".cargo"
        || lower == "pip"
        || lower == "uv"
        || lower == ".m2"
        || lower == "pnpm"
        || lower == "yarn"
    {
        return Category::PackageCache;
    }

    // 4. SDK 与系统开发库
    if lower.contains("android") || lower.contains("xcode") || lower.contains("deriveddata") {
        return Category::Sdk;
    }

    // 5. 文档与媒体
    if lower == "documents"
        || lower == "pictures"
        || lower == "music"
        || lower == "videos"
        || lower == "photos library"
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".mp4")
        || lower.ends_with(".mov")
        || lower.ends_with(".mp3")
        || lower.ends_with(".wav")
        || lower.ends_with(".pdf")
        || lower.ends_with(".docx")
        || lower.ends_with(".xlsx")
    {
        return Category::DocumentMedia;
    }

    // 6. 开发工具与其它隐式文件夹
    if lower == ".vscode"
        || lower == ".cursor"
        || lower == ".idea"
        || lower == ".git"
        || lower == ".docker"
        || lower == ".wsl"
    {
        return Category::DevToolOther;
    }

    Category::Other
}
