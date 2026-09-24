import { useEffect, useState, useMemo } from 'react';
import { useAccountStore } from '../stores/useAccountStore';
import { request } from '../utils/request';
import { Gemini, Claude, OpenAI } from '@lobehub/icons';
import { Bot } from 'lucide-react';

export interface DynamicModelOption {
    id: string;
    label: string;
    shortLabel: string;
    group: string;
    Icon: any;
    recommended?: boolean;
    supportsThinking?: boolean;
    supportsImages?: boolean;
}

interface BackendDiscoveredModel {
    id: string;
    display_name: string;
    short_label: string;
    group: string;
    supports_thinking: boolean;
    supports_images: boolean;
    recommended: boolean;
    last_seen: number;
}

/**
 * 自动根据模型 ID 选择合适的展示图标组件
 */
export function getModelIconComponent(modelId: string) {
    const id = modelId.toLowerCase();
    if (id.startsWith('gemini')) return Gemini.Color;
    if (id.startsWith('claude')) return Claude.Color;
    if (id.startsWith('gpt') || id.startsWith('o1') || id.startsWith('o3')) return OpenAI.Avatar;
    return Bot;
}

/**
 * 自动推断模型的系列分组 (Group)
 */
export function getModelGroup(modelId: string): string {
    const lower = modelId.toLowerCase();
    if (lower.startsWith('gemini-3')) return 'Gemini 3';
    if (lower.startsWith('gemini-2.5')) return 'Gemini 2.5';
    if (lower.startsWith('gemini-2')) return 'Gemini 2';
    if (lower.startsWith('gemini')) return 'Gemini';
    if (lower.startsWith('claude')) return 'Claude';
    if (lower.startsWith('gpt')) return 'OpenAI';
    return 'Other';
}

/**
 * 自动推断或精炼模型的短标签 (如 "G3.8 Flash", "Claude 3.5 Sonnet")
 */
export function getModelShortLabel(modelId: string, displayName?: string): string {
    if (displayName && !displayName.toLowerCase().includes('dynamic extracted model') && displayName !== modelId) {
        if (displayName.startsWith('Gemini ')) {
            return `G${displayName.slice(7)}`;
        }
        return displayName;
    }

    const lower = modelId.toLowerCase();
    if (lower.includes('claude-sonnet-4-6')) return 'Claude 4.6';
    if (lower.includes('claude-opus-4-6')) return 'Claude Opus 4.6';
    if (lower.includes('claude-3-5-sonnet')) return 'Claude 3.5';
    if (lower.includes('gpt-oss')) return 'GPT-OSS';

    if (lower.startsWith('gemini-')) {
        const parts = lower.replace('gemini-', '').split('-');
        if (parts.length > 0) {
            const ver = parts[0];
            const rest = parts.slice(1).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join(' ');
            return `G${ver} ${rest}`.trim();
        }
    }

    return modelId;
}

/**
 * 全局共享的动态模型 Hook
 * 自动汇总：
 * 1. 后端 /api/models/available 持久化已探测到的共用模型池
 * 2. 当前已加载的所有账号配额数据中实际拥有的模型
 */
export function useDynamicModels(): DynamicModelOption[] {
    const { accounts } = useAccountStore();
    const [backendModels, setBackendModels] = useState<BackendDiscoveredModel[]>([]);

    useEffect(() => {
        let mounted = true;
        request<BackendDiscoveredModel[]>('get_available_models')
            .then((res) => {
                if (mounted && Array.isArray(res)) {
                    setBackendModels(res);
                }
            })
            .catch(() => {
                // 后端未启动或离线时平滑忽略
            });
        return () => {
            mounted = false;
        };
    }, []);

    const modelOptions = useMemo(() => {
        const modelMap = new Map<string, DynamicModelOption>();

        // 1. 优先放入后端探测到的共用模型
        backendModels.forEach((bm) => {
            const id = bm.id.toLowerCase();
            if (id.includes('thinking')) return; // 隐藏单纯的思维中间件变体
            modelMap.set(id, {
                id: bm.id,
                label: bm.display_name || bm.id,
                shortLabel: bm.short_label || getModelShortLabel(bm.id, bm.display_name),
                group: bm.group || getModelGroup(bm.id),
                Icon: getModelIconComponent(bm.id),
                recommended: bm.recommended,
                supportsThinking: bm.supports_thinking,
                supportsImages: bm.supports_images,
            });
        });

        // 2. 结合当前所有账号配额中实际包含的模型补充（防止新账号有尚未同步到共用池的私有模型）
        accounts.forEach((acc) => {
            const models = acc.quota?.models || [];
            models.forEach((m) => {
                const id = m.name.toLowerCase();
                if (id.includes('thinking')) return;
                const existing = modelMap.get(id);
                const displayName = m.display_name || existing?.label;

                if (!existing) {
                    modelMap.set(id, {
                        id: m.name,
                        label: displayName || m.name,
                        shortLabel: getModelShortLabel(m.name, displayName),
                        group: getModelGroup(m.name),
                        Icon: getModelIconComponent(m.name),
                        supportsThinking: m.supports_thinking,
                        supportsImages: m.supports_images,
                        recommended: m.recommended,
                    });
                } else if (m.display_name && existing.label === existing.id) {
                    existing.label = m.display_name;
                    existing.shortLabel = getModelShortLabel(m.name, m.display_name);
                }
            });
        });

        // 3. 排序：按分组优先级，再按模型名称自然排序
        return Array.from(modelMap.values()).sort((a, b) => {
            const groupOrder: Record<string, number> = {
                'Gemini 3': 10,
                'Gemini 2.5': 20,
                'Gemini 2': 30,
                'Gemini': 40,
                'Claude': 50,
                'OpenAI': 60,
                'Other': 70,
            };
            const gA = groupOrder[a.group] ?? 99;
            const gB = groupOrder[b.group] ?? 99;
            if (gA !== gB) return gA - gB;
            return a.id.localeCompare(b.id);
        });
    }, [backendModels, accounts]);

    return modelOptions;
}
