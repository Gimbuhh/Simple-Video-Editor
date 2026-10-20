// Test-only oracle: references the unchanged production baseline. The Rust
// executable never loads this assembly and has no C# runtime dependency.
using System.Text.Json;
using SimpleVideoEditor;
using SimpleVideoEditor.Services;

if (args.Length != 1) throw new ArgumentException("Expected one schema-1 fixture path.");
var document = await ProjectStore.ReadAsync(args[0]);
var sources = ProjectStore.SourcePaths(document).ToArray();
var clips = document.Clips.Select((c, index) => new MediaClip
{
    Path = c.Path, Start = c.Start, End = c.End, TimelineStart = c.TimelineStart
}).ToArray();
Console.WriteLine(JsonSerializer.Serialize(new
{
    schemaVersion = document.Version,
    sourceCount = sources.Length,
    clipCount = clips.Length,
    durationSeconds = TimelineLayout.Duration(clips),
    clips = clips.Select((c, index) => new
    {
        clipId = index + 1,
        sourceId = Array.FindIndex(sources, p => StringComparer.OrdinalIgnoreCase.Equals(p, c.Path)) + 1,
        sourceIn = c.Start,
        sourceOut = c.End,
        timelineStart = TimelineLayout.Offset(clips, c)
    })
}));
