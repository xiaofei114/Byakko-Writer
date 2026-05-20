# 写作风格分析提示词

## 任务
请分析以下小说文本的写作风格，并以 JSON 格式返回分析结果。

## 需要分析的维度

1. **语言风格**：整体语调、用词特点、句式结构、修辞密度
2. **叙事特征**：叙事视角、时序安排、叙事距离、心理描写深度
3. **对话风格**：对话密度、对话特点、对话标签使用、方言使用情况
4. **描写偏好**：五感使用、环境描写比重、动作描写风格、心理描写风格
5. **节奏特点**：整体节奏、段落长度偏好、场景切换频率
6. **常用修辞手法**：列举3-5个
7. **典型句式特征**：列举3-5个
8. **词汇偏好**：高频词、特色用词、词汇难度

## 文本内容

{text_content}

## 输出格式

请以以下 JSON 格式返回：

```json
{
    "language_style": {
        "overall_tone": "简洁/华丽/朴实/诗意/冷峻等",
        "word_choice": "用词特点描述",
        "sentence_structure": "句式特点",
        "rhetoric_density": "高/中/低"
    },
    "narrative_features": {
        "perspective": "第一人称/第三人称限知/第三人称全知等",
        "chronology": "线性/倒叙/插叙/多线",
        "narrative_distance": "沉浸/疏离/客观",
        "psychological_depth": "深入/适中/浅显"
    },
    "dialogue_style": {
        "dialogue_density": "高/中/低",
        "dialogue_characteristics": "简洁/幽默/正式/口语化等",
        "dialogue_tag_style": "对话标签使用习惯",
        "dialect_usage": "方言使用情况"
    },
    "description_preference": {
        "sensory_preference": ["视觉", "听觉"],
        "environment_description_level": "高/中/低",
        "action_description_style": "动作描写风格",
        "psychological_description_style": "心理描写风格"
    },
    "pacing_style": {
        "overall_pacing": "紧凑/舒缓/张弛有度",
        "paragraph_length_preference": "偏好长段落/短段落/混合",
        "scene_transition_frequency": "频繁/适中/较少"
    },
    "common_rhetoric": ["比喻", "拟人", "排比"],
    "sentence_patterns": ["四字短语", "长句铺陈"],
    "vocabulary_preference": {
        "high_frequency_words": ["高频词1", "高频词2"],
        "characteristic_words": ["特色词1", "特色词2"],
        "avoided_words": ["少用词1"],
        "vocabulary_level": "通俗/中等/艰深"
    }
}
```
