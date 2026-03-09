#!/usr/bin/env python3
import os
import sys
import json
from datetime import datetime
from pathlib import Path

# 检查维度
CHECK_DIMENSIONS = [
    "安全性",
    "高性能",
    "高解耦",
    "功能完善必要性",
    "用户体验感"
]

# 优先级分类
PRIORITY_CATEGORIES = [
    "必要且紧急",
    "紧急但不必要",
    "必要但不紧急",
    "不必也不紧急"
]

def analyze_security(repo_path):
    """检查安全性"""
    issues = []
    # 检查敏感文件
    sensitive_files = [".env", "config.json", "secret.key", "token"]
    for root, _, files in os.walk(repo_path):
        for file in files:
            if any(s in file.lower() for s in sensitive_files):
                issues.append(f"发现潜在敏感文件：{os.path.join(root, file)}")
    
    # 检查代码中的硬编码密钥
    for ext in [".rs", ".toml", ".yaml", ".yml", ".json"]:
        for file in Path(repo_path).rglob(f"*{ext}"):
            try:
                content = file.read_text()
                if "api_key" in content.lower() or "secret" in content.lower() or "token" in content.lower():
                    if "=" in content or ":" in content:
                        issues.append(f"代码中可能存在硬编码密钥：{file}")
            except:
                pass
    
    return issues or ["未发现明显安全问题"]

def analyze_performance(repo_path):
    """检查高性能"""
    issues = []
    # 检查Cargo.toml中的优化配置
    cargo_toml = Path(repo_path) / "Cargo.toml"
    if cargo_toml.exists():
        content = cargo_toml.read_text()
        if "[profile.release]" not in content:
            issues.append("缺少Release版本优化配置")
        if "opt-level" not in content:
            issues.append("未配置编译优化级别")
    
    # 检查代码中的潜在性能问题
    for file in Path(repo_path).rglob("*.rs"):
        try:
            content = file.read_text()
            if "unwrap()" in content:
                issues.append(f"代码中使用unwrap()可能导致panic：{file}")
            if "clone()" in content:
                issues.append(f"代码中使用clone()可能有性能优化空间：{file}")
        except:
            pass
    
    return issues or ["未发现明显性能问题"]

def analyze_decoupling(repo_path):
    """检查高解耦"""
    issues = []
    src_dir = Path(repo_path) / "src"
    if src_dir.exists():
        modules = [d for d in src_dir.iterdir() if d.is_dir()]
        if len(modules) < 2:
            issues.append("代码模块划分不足，可能存在耦合问题")
        
        # 检查模块间依赖
        for module in modules:
            for file in module.rglob("*.rs"):
                try:
                    content = file.read_text()
                    imports = [line for line in content.split("\n") if line.startswith("use crate::")]
                    cross_module_imports = [imp for imp in imports if not imp.startswith(f"use crate::{module.name}")]
                    if len(cross_module_imports) > 5:
                        issues.append(f"模块{module.name}跨模块依赖过多，可能耦合度较高")
                except:
                    pass
    
    return issues or ["模块解耦情况良好"]

def analyze_function_completeness(repo_path):
    """检查功能完善必要性"""
    issues = []
    readme = Path(repo_path) / "README.md"
    if readme.exists():
        content = readme.read_text()
        if len(content) < 1000:
            issues.append("README文档不够完善，需要补充功能说明")
    
    # 检查测试覆盖率
    tests_dir = Path(repo_path) / "tests"
    if not tests_dir.exists() or len(list(tests_dir.rglob("*.rs"))) == 0:
        issues.append("缺少测试用例，功能稳定性无法保证")
    
    # 检查错误处理
    for file in Path(repo_path).rglob("*.rs"):
        try:
            content = file.read_text()
            if "?" not in content and "Result" not in content:
                issues.append(f"文件{file}缺少错误处理")
        except:
            pass
    
    return issues or ["功能完整性良好"]

def analyze_user_experience(repo_path):
    """检查用户体验感"""
    issues = []
    # 检查CLI帮助信息
    for file in Path(repo_path).rglob("*.rs"):
        try:
            content = file.read_text()
            if "clap" in content or "cli" in content.lower():
                if "help" not in content.lower() or "version" not in content.lower():
                    issues.append("CLI工具缺少帮助信息或版本参数")
        except:
            pass
    
    # 检查文档
    docs_dir = Path(repo_path) / "docs"
    if not docs_dir.exists():
        issues.append("缺少用户文档目录")
    
    return issues or ["用户体验设计良好"]

def prioritize_issues(all_issues):
    """将问题按优先级分类"""
    prioritized = {cat: [] for cat in PRIORITY_CATEGORIES}
    
    for dimension, issues in all_issues.items():
        for issue in issues:
            # 简单的优先级判断逻辑，可根据实际情况调整
            if "安全" in issue or "panic" in issue or "错误处理" in issue:
                prioritized["必要且紧急"].append(f"[{dimension}] {issue}")
            elif "性能" in issue or "优化" in issue:
                prioritized["必要但不紧急"].append(f"[{dimension}] {issue}")
            elif "文档" in issue or "体验" in issue:
                prioritized["紧急但不必要"].append(f"[{dimension}] {issue}")
            else:
                prioritized["不必也不紧急"].append(f"[{dimension}] {issue}")
    
    return prioritized

def generate_requirement_doc(prioritized_issues, output_path):
    """生成需求文档"""
    today = datetime.now().strftime("%Y-%m-%d")
    doc_content = f"# 需求分析报告 - {today}\n\n"
    doc_content += "## 优先级分类\n\n"
    
    for category in PRIORITY_CATEGORIES:
        issues = prioritized_issues[category]
        if issues:
            doc_content += f"### {category}\n"
            for i, issue in enumerate(issues, 1):
                doc_content += f"{i}. [ ] {issue}\n"
            doc_content += "\n"
    
    doc_content += "## 分析说明\n"
    doc_content += "- 本报告基于代码静态分析生成，仅供参考\n"
    doc_content += "- 请审核后勾选需要实现的需求点\n"
    
    with open(output_path, "w", encoding="utf-8") as f:
        f.write(doc_content)
    
    return doc_content

def main():
    repo_path = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    today = datetime.now().strftime("%Y-%m-%d")
    output_dir = os.path.join(repo_path, "requirements")
    os.makedirs(output_dir, exist_ok=True)
    
    # 执行各维度检查
    print("开始仓库分析...")
    all_issues = {
        "安全性": analyze_security(repo_path),
        "高性能": analyze_performance(repo_path),
        "高解耦": analyze_decoupling(repo_path),
        "功能完善必要性": analyze_function_completeness(repo_path),
        "用户体验感": analyze_user_experience(repo_path)
    }
    
    # 优先级分类
    prioritized = prioritize_issues(all_issues)
    
    # 生成文档
    output_file = os.path.join(output_dir, f"requirements-{today}.md")
    doc_content = generate_requirement_doc(prioritized, output_file)
    
    print(f"需求分析完成！文档已生成：{output_file}")
    print("\n" + "="*50 + "\n")
    print(doc_content)
    return 0

if __name__ == "__main__":
    sys.exit(main())
