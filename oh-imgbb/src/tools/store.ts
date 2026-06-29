import { create } from 'zustand';
import type { LoginStatus, SearchPing } from '../api/types';

type AppState = {
    darkMode: boolean;
    loginStatus: LoginStatus;
    webSearchText: string;
    webSearchPing?: SearchPing;
    webSearchPingLoading: boolean;
    loadedWebSearchDetect: boolean;

    setState: (partial: Partial<AppState>) => void;
};

export const useAppStore = create<AppState>((set) => ({
    darkMode: false,
    loginStatus: { logged_in: false, verified: false },
    webSearchText: "",
    webSearchPing: undefined,
    webSearchPingLoading: false,
    loadedWebSearchDetect: false,
    setState: (partial) => set(partial),
}));
