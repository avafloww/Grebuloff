using System.IO.Pipes;

namespace Grebuloff.Dalamud;

public class Channel : IDisposable
{
    public string Name { get; }
    private readonly CancellationTokenSource _cts = new();
    private readonly NamedPipeServerStream _server;
    private StreamWriter? _writer;
    private StreamReader? _reader;

    public Channel()
    {
        // generate a random UUID
        Name = $"grebuloff-dalamud-{Guid.NewGuid()}";
        _server = new NamedPipeServerStream(
            Name,
            PipeDirection.InOut,
            1,
            PipeTransmissionMode.Message,
            PipeOptions.Asynchronous | PipeOptions.CurrentUserOnly
        );
    }

    private async void AwaitConnection()
    {
        await _server.WaitForConnectionAsync(_cts.Token);
        _writer = new StreamWriter(_server);
        _reader = new StreamReader(_server);
    }

    public void Dispose()
    {
        _cts.Cancel();
        _reader?.Dispose();
        _writer?.Dispose();
        _server.Dispose();
    }
}