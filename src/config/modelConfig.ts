import {
    getModelIconComponent,
    getModelGroup,
    getModelShortLabel,
    useDynamicModels,
    type DynamicModelOption,
} from '../hooks/useDynamicModels';
import { getModelProtectionKey } from '../utils/modelCategory';

/**
 * 模型配置接口
 */
export interface ModelConfig {
    /** 模型完整显示名称 */
    label: string;
    /** 模型简短标签 */
    shortLabel: string;
    /** 保护模型的键名 */
    protectedKey: string;
    /** 模型图标组件 */
    Icon: any;
    /** 国际化键名 (可选) */
    i18nKey: string;
    /** 描述信息键名 (可选) */
    i18nDescKey: string;
    /** 所属系列/分组 */
    group: string;
    /** 选填标签 (用于筛选) */
    tags?: string[];
}

/**
 * 动态根据模型 ID 实时构建 ModelConfig（不再硬编码模型列表）
 */
export function createDynamicModelConfig(modelId: string): ModelConfig {
    const shortLabel = getModelShortLabel(modelId);
    return {
        label: modelId,
        shortLabel,
        protectedKey: getModelProtectionKey(modelId) || modelId,
        Icon: getModelIconComponent(modelId),
        i18nKey: '',
        i18nDescKey: '',
        group: getModelGroup(modelId),
        tags: [],
    };
}

/**
 * 动态模型配置映射（基于 Proxy 动态响应任意模型 ID，彻底删除写死的清单）
 */
export const MODEL_CONFIG: Record<string, ModelConfig> = new Proxy(
    {} as Record<string, ModelConfig>,
    {
        get(target, prop: string) {
            if (typeof prop !== 'string') return undefined;
            if (prop in target) return target[prop];
            const cfg = createDynamicModelConfig(prop);
            target[prop] = cfg;
            return cfg;
        },
        has() {
            return true;
        },
    }
);

/**
 * 获取所有已探测并缓存的模型 ID 列表
 */
export const getAllModelIds = (): string[] => Object.keys(MODEL_CONFIG);

/**
 * 根据模型 ID 动态获取配置
 */
export const getModelConfig = (modelId: string): ModelConfig => {
    return MODEL_CONFIG[modelId.toLowerCase()];
};

/**
 * 模型排序权重配置
 */
const MODEL_SORT_WEIGHTS = {
    series: {
        'gemini-3': 100,
        'gemini-2.5': 200,
        'gemini-2': 300,
        'claude': 400,
    },
    tier: {
        'pro': 10,
        'flash': 20,
        'lite': 30,
        'opus': 5,
        'sonnet': 10,
    },
    suffix: {
        'thinking': 1,
        'image': 2,
        'high': 0,
        'low': 3,
    },
};

/**
 * 获取模型的排序权重
 */
function getModelSortWeight(modelId: string): number {
    const id = modelId.toLowerCase();
    let weight = 0;

    if (id.startsWith('gemini-3')) {
        weight += MODEL_SORT_WEIGHTS.series['gemini-3'] * 1000;
    } else if (id.startsWith('gemini-2.5')) {
        weight += MODEL_SORT_WEIGHTS.series['gemini-2.5'] * 1000;
    } else if (id.startsWith('gemini-2')) {
        weight += MODEL_SORT_WEIGHTS.series['gemini-2'] * 1000;
    } else if (id.startsWith('claude')) {
        weight += MODEL_SORT_WEIGHTS.series['claude'] * 1000;
    }

    if (id.includes('pro')) {
        weight += MODEL_SORT_WEIGHTS.tier['pro'] * 100;
    } else if (id.includes('flash')) {
        weight += MODEL_SORT_WEIGHTS.tier['flash'] * 100;
    } else if (id.includes('lite')) {
        weight += MODEL_SORT_WEIGHTS.tier['lite'] * 100;
    } else if (id.includes('opus')) {
        weight += MODEL_SORT_WEIGHTS.tier['opus'] * 100;
    } else if (id.includes('sonnet')) {
        weight += MODEL_SORT_WEIGHTS.tier['sonnet'] * 100;
    }

    if (id.includes('thinking')) {
        weight += MODEL_SORT_WEIGHTS.suffix['thinking'] * 10;
    } else if (id.includes('image')) {
        weight += MODEL_SORT_WEIGHTS.suffix['image'] * 10;
    } else if (id.includes('high')) {
        weight += MODEL_SORT_WEIGHTS.suffix['high'] * 10;
    } else if (id.includes('low')) {
        weight += MODEL_SORT_WEIGHTS.suffix['low'] * 10;
    }

    return weight;
}

/**
 * 对模型列表进行排序
 */
export function sortModels<T extends { id: string }>(models: T[]): T[] {
    return [...models].sort((a, b) => {
        const weightA = getModelSortWeight(a.id);
        const weightB = getModelSortWeight(b.id);
        if (weightA !== weightB) {
            return weightA - weightB;
        }
        return a.id.localeCompare(b.id);
    });
}

// 导出动态模型相关接口
export { useDynamicModels, type DynamicModelOption };

// Re-export 分类与显示工具函数
export {
    categorizeModel,
    getModelProtectionKey,
    getModelDisplayName,
    findQuotaModel,
    findImageQuotaModel,
    ensurePinnedImageSelector,
    DEFAULT_IMAGE_PIN_SELECTOR,
    resolveQuotaModels,
    type ModelCategory,
    type QuotaModelSelection,
} from '../utils/modelCategory';
