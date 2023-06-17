using Dalamud.Game;
using Dalamud.Plugin;

namespace Grebuloff.Dalamud;

public sealed class Plugin : IDalamudPlugin
{
    public string Name => "Grebuloff Compatibility Layer";

    public DalamudPluginInterface PluginInterface { get; private set; }
    public Framework Framework { get; private set; }
    
    public Plugin(DalamudPluginInterface pluginInterface, Framework framework)
    {
        PluginInterface = pluginInterface;
        Framework = framework;

        PluginInterface.UiBuilder.Draw += this.DrawUI;
        Framework.Update += OnFrameworkUpdate;
    }

    private void OnFrameworkUpdate(Framework framework)
    {
    }

    private void DrawUI()
    {
        
    }
    
    public void Dispose()
    {
        Framework.Update -= OnFrameworkUpdate;
        PluginInterface.UiBuilder.Draw -= this.DrawUI;
    }
}