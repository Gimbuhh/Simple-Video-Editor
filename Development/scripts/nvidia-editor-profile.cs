// NVAPI ABI: https://github.com/NVIDIA/nvapi (nvapi.h, nvapi_interface.h).
// This opt-in maintenance tool is not loaded by the Rust application.
using System;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace SVE {
public static class NvidiaCaptureProfile {
    // Observed in NVIDIA's built-in OBS, VLC and Blender profiles. Undocumented
    // by NVIDIA; do not confuse it with a supported application-local opt-out.
    const uint OverlayFlags = 0x809d5f60, NonGame = 0x10000000;
    const int AppNotFound = -166;
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    struct Application {
        public uint version, predefined;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=2048)] public string name;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=2048)] public string friendly;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=2048)] public string launcher;
    }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    struct Profile {
        public uint version;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=2048)] public string name;
        public uint gpu, predefined, apps, settings;
    }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode, Pack=4)]
    struct Setting {
        public uint version;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=2048)] public string name;
        public uint id, type, location, predefined, valid;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst=4100)] public byte[] defaultValue;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst=4100)] public byte[] current;
    }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr LoadLibraryExW(string name, IntPtr file, uint flags);
    [DllImport("kernel32.dll", CharSet=CharSet.Ansi, ExactSpelling=true)]
    static extern IntPtr GetProcAddress(IntPtr module, string name);
    [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate IntPtr Query(uint id);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int NoArgs();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int Create(out IntPtr session);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int Session(IntPtr session);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet=CharSet.Unicode)]
    delegate int Find(IntPtr session, string name, out IntPtr profile, ref Application app);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int GetProfile(IntPtr session, IntPtr profile, ref Profile info);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int CreateProfile(IntPtr session, ref Profile info, out IntPtr profile);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int CreateApplication(IntPtr session, IntPtr profile, ref Application app);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int GetSetting(IntPtr session, IntPtr profile, uint id, ref Setting setting);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int SetSetting(IntPtr session, IntPtr profile, ref Setting setting);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int DeleteProfile(IntPtr session, IntPtr profile);

    static T Function<T>(Query query, uint id) where T: Delegate {
        var address=query(id);
        if(address==IntPtr.Zero) throw new InvalidOperationException("NVIDIA API unavailable: 0x"+id.ToString("x8"));
        return Marshal.GetDelegateForFunctionPointer<T>(address);
    }
    static uint Version<T>() { return (uint)Marshal.SizeOf<T>() | 0x10000; }
    static bool SamePath(string left, string right) {
        // DRS returns lowercase, forward-slash paths even for a backslash input.
        return String.Equals(left.Replace('\\','/'),right.Replace('\\','/'),StringComparison.OrdinalIgnoreCase);
    }
    static void Check(int status) {
        if(status==-137) throw new InvalidOperationException("NVIDIA requires Windows administrator permission to save this per-editor profile. Run this maintenance script in an elevated PowerShell; the editor itself stays unelevated.");
        if(status!=0) throw new InvalidOperationException("NVIDIA profile operation failed ("+status+"). No global settings were requested.");
    }
    static Application NewApp() {
        return new Application {version=Version<Application>(), name="", friendly="", launcher=""};
    }
    static Setting NewSetting() {
        return new Setting {version=Version<Setting>(), name="", id=OverlayFlags,
            defaultValue=new byte[4100], current=new byte[4100]};
    }
    public static string Run(string path, string ownedName, string action) {
        if(action!="Inspect" && action!="Exclude" && action!="Restore") throw new ArgumentException("Unknown action");
        // Restrict loading to System32, never the project or current directory.
        var library=LoadLibraryExW("nvapi64.dll", IntPtr.Zero, 0x800);
        if(library==IntPtr.Zero) throw new InvalidOperationException("NVIDIA x64 driver API is not installed.");
        IntPtr session=IntPtr.Zero;
        Query query=null;
        bool initialized=false;
        try {
            var address=GetProcAddress(library,"nvapi_QueryInterface");
            if(address==IntPtr.Zero) throw new InvalidOperationException("NVIDIA query interface unavailable.");
            query=Marshal.GetDelegateForFunctionPointer<Query>(address);
            Check(Function<NoArgs>(query,0x0150e828)()); initialized=true;
            Check(Function<Create>(query,0x0694d52e)(out session));
            var load=Function<Session>(query,0x375dbd6b);
            var find=Function<Find>(query,0xeee566b2);
            var getProfile=Function<GetProfile>(query,0x61cd6fd6);
            var getSetting=Function<GetSetting>(query,0x73bf8338);
            Check(load(session));
            var app=NewApp(); IntPtr profile;
            int found=find(session,path,out profile,ref app);
            if(found!=0 && found!=AppNotFound) Check(found);
            var info=new Profile {version=Version<Profile>(), name=""};
            var setting=NewSetting();
            int settingStatus=-160;
            if(found==0) {
                Check(getProfile(session,profile,ref info));
                settingStatus=getSetting(session,profile,OverlayFlags,ref setting);
            }
            bool excluded=settingStatus==0 && setting.type==0 && (BitConverter.ToUInt32(setting.current,0)&NonGame)!=0;
            bool owned=found==0 && info.name==ownedName && info.predefined==0 && info.apps==1 && info.settings==1
                && app.predefined==0 && SamePath(app.name,path)
                && settingStatus==0 && setting.type==0 && setting.location==0 && setting.predefined==0
                && BitConverter.ToUInt32(setting.current,0)==NonGame;
            if(action=="Inspect") return JsonSerializer.Serialize(new {path, profile=found==0?info.name:null, excluded, owned});
            if(action=="Restore") {
                if(found==AppNotFound) return JsonSerializer.Serialize(new {path, restored=true, changed=false});
                if(!owned) throw new InvalidOperationException("This is not the unchanged profile created by this tool; it was preserved.");
                Check(Function<DeleteProfile>(query,0x17093206)(session,profile));
            } else {
                if(excluded) return JsonSerializer.Serialize(new {path, profile=info.name, excluded=true, owned, changed=false});
                if(found==0) throw new InvalidOperationException("An existing NVIDIA profile covers this editor; it was preserved. Inspect it manually.");
                info=new Profile {version=Version<Profile>(),name=ownedName};
                Check(Function<CreateProfile>(query,0xcc176068)(session,ref info,out profile));
                app=NewApp(); app.name=path; app.friendly="Simple Video Editor";
                Check(Function<CreateApplication>(query,0x4347a9de)(session,profile,ref app));
                setting=NewSetting(); Array.Copy(BitConverter.GetBytes(NonGame),setting.current,4);
                Check(Function<SetSetting>(query,0x577dd202)(session,profile,ref setting));
            }
            // Only our single-app profile is changed in this session. Errors before
            // SaveSettings discard the uncommitted session; no global fallback.
            Check(Function<Session>(query,0xfcbc7e14)(session));
            Check(load(session));
            app=NewApp();
            found=find(session,path,out profile,ref app);
            if(action=="Restore") {
                if(found!=AppNotFound) throw new InvalidOperationException("Profile restoration did not verify.");
                return JsonSerializer.Serialize(new {path,restored=true,changed=true});
            }
            Check(found);
            info=new Profile {version=Version<Profile>(),name=""};
            Check(getProfile(session,profile,ref info));
            setting=NewSetting(); Check(getSetting(session,profile,OverlayFlags,ref setting));
            if(info.name!=ownedName || info.apps!=1 || info.settings!=1 || setting.type!=0 || setting.location!=0
                || BitConverter.ToUInt32(setting.current,0)!=NonGame || !SamePath(app.name,path))
                throw new InvalidOperationException("Saved editor exclusion did not verify; inspect the profile before retrying.");
            return JsonSerializer.Serialize(new {path,profile=info.name,excluded=true,owned=true,changed=true});
        } finally {
            if(session!=IntPtr.Zero) Function<Session>(query,0xdad9cff8)(session);
            if(initialized) Function<NoArgs>(query,0xd22bdd7e)();
            FreeLibrary(library);
        }
    }
}
}
