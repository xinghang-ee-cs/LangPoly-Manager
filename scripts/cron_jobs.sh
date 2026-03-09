#!/bin/bash

# 加载用户环境变量
export PATH="/root/.local/share/pnpm:$PATH"
export HOME="/root"

# 项目路径
PROJECT_DIR="/root/.openclaw/workspace/LangPoly-Manager"
LOG_FILE="$PROJECT_DIR/requirements/cron_$(date +%Y-%m-%d).log"

# 重定向所有输出到日志文件
exec >> "$LOG_FILE" 2>&1
echo "[$(date)] 开始执行任务：$1"

# 早上6点：执行仓库分析，生成需求文档
run_analysis() {
    echo "[$(date)] 开始执行仓库分析任务（调用OpenClaw Agent）..."
    cd "$PROJECT_DIR" || exit 1
    
    # 调用OpenClaw Agent执行代码分析任务
    TASK="请对当前目录的Rust项目进行静态代码分析，按以下优先级分类生成需求文档：
1. 必要且紧急：安全问题、可能导致panic的代码、缺少错误处理的文件
2. 必要但不紧急：性能优化、编译配置优化
3. 不必也不紧急：代码解耦、用户体验优化
输出格式要求和之前的需求分析报告完全一致，保存到 requirements/requirements-$(date +%Y-%m-%d).md 文件。"
    
    echo "[$(date)] 向Agent提交分析任务..."
    if openclaw sessions_spawn --task "$TASK" --runtime acp --mode run --cwd "$PROJECT_DIR"; then
        echo "[$(date)] 代码分析完成"
        
        # 发送文档内容给用户（飞书通知）
        TODAY=$(date +%Y-%m-%d)
        REQ_FILE="$PROJECT_DIR/requirements/requirements-$TODAY.md"
        echo "[$(date)] 检查需求文件：$REQ_FILE"
        if [ -f "$REQ_FILE" ]; then
            echo "[$(date)] 需求文档生成完成：$REQ_FILE"
            # 发送飞书消息
            CONTENT=$(cat "$REQ_FILE")
            echo "[$(date)] 准备发送需求报告，消息长度：${#CONTENT}"
            if openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "📊 今日需求分析报告已生成：

$CONTENT"; then
                echo "[$(date)] 报告发送成功"
            else
                echo "[$(date)] 报告发送失败，错误代码：$?"
                # 重试一次
                sleep 2
                openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "⚠️ 重试：今日需求分析报告已生成（${#CONTENT}字）"
            fi
        else
            echo "[$(date)] 错误：需求文件未生成"
            openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "❌ 需求分析失败：未生成报告文件"
        fi
    else
        echo "[$(date)] 代码分析任务执行失败，错误代码：$?"
        openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "❌ 需求分析任务执行失败，请检查日志"
    fi
}

# 上午10点：执行已批准的需求
run_implementation() {
    echo "[$(date)] 开始执行已批准的需求（调用OpenClaw Agent）..."
    cd "$PROJECT_DIR" || exit 1
    
    TODAY=$(date +%Y-%m-%d)
    REQ_FILE="$PROJECT_DIR/requirements/requirements-$TODAY.md"
    
    if [ ! -f "$REQ_FILE" ]; then
        echo "[$(date)] 错误：今日需求文件不存在"
        openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "❌ 需求执行失败：未找到今日需求文件"
        exit 1
    fi
    
    # 调用OpenClaw Agent执行需求实现任务
    TASK="请读取今日需求文件 $REQ_FILE，找出所有已经标记为 [x] 或 [✓] 的需求点，逐个实现。
实现要求：
1. 优先处理必要且紧急的需求
2. 每个需求实现后要确保代码能正常编译
3. 实现完成后生成执行报告，包含完成的需求数量、失败的需求数量和具体原因
报告保存到 requirements/implementation_$(date +%Y-%m-%d).log 文件。"
    
    echo "[$(date)] 向Agent提交需求实现任务..."
    if openclaw sessions_spawn --task "$TASK" --runtime acp --mode run --cwd "$PROJECT_DIR"; then
        echo "[$(date)] 需求执行完成"
        
        # 发送执行结果给用户
        LOG_PATH="$PROJECT_DIR/requirements/implementation_$(date +%Y-%m-%d).log"
        RESULT=$(tail -30 "$LOG_PATH")
        echo "[$(date)] 准备发送执行结果，消息长度：${#RESULT}"
        if openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "✅ 今日需求执行完成：

$RESULT"; then
            echo "[$(date)] 执行结果发送成功"
        else
            echo "[$(date)] 执行结果发送失败，错误代码：$?"
            # 重试一次
            sleep 2
            openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "⚠️ 重试：今日需求执行完成，详情见日志 $LOG_PATH"
        fi
    else
        echo "[$(date)] 需求执行任务失败，错误代码：$?"
        openclaw message send --channel feishu --target user:ou_95784e1b9fa0ace2bbdd23a02b829cff --message "❌ 需求执行任务失败，请检查日志"
    fi
}

# 根据参数执行对应功能
case "$1" in
    analysis)
        run_analysis
        ;;
    implementation)
        run_implementation
        ;;
    *)
        echo "Usage: $0 {analysis|implementation}"
        exit 1
        ;;
esac
