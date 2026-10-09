namespace SimpleVideoEditor;

public static class TimelineLayout
{
    public static double Offset(IEnumerable<MediaClip> clips, MediaClip target)
    {
        if (target.TimelineStart is { } position) return position;
        double end = 0;
        foreach (var clip in clips)
        {
            var start = clip.TimelineStart ?? end;
            if (clip == target) return start;
            end = start + clip.KeptDuration;
        }
        return 0;
    }
    public static double Duration(IEnumerable<MediaClip> clips)
    {
        double end = 0, maximum = 0;
        foreach (var clip in clips) { end = (clip.TimelineStart ?? end) + clip.KeptDuration; maximum = Math.Max(maximum, end); }
        return maximum;
    }
    public static double AvailablePosition(IEnumerable<MediaClip> clips, MediaClip moving, double requested, double snapDistance = 0)
    {
        var ranges = clips.Where(c => c != moving).Select(c => (Start: Offset(clips, c), End: Offset(clips, c) + c.KeptDuration)).OrderBy(c => c.Start).ToArray();
        var snap = ranges.SelectMany(c => new[] { c.End, c.Start - moving.KeptDuration }).Append(0d)
            .Where(p => p >= 0 && Math.Abs(p - requested) <= snapDistance && ranges.All(c => p + moving.KeptDuration <= c.Start + 1e-8 || p >= c.End - 1e-8))
            .OrderBy(p => Math.Abs(p - requested)).Select(p => (double?)p).FirstOrDefault();
        if (snap is { } boundary) return boundary;
        requested = Math.Max(0, Math.Round(requested * moving.FrameRate) / moving.FrameRate);
        double end = 0;
        var candidates = new List<double>();
        foreach (var range in ranges)
        {
            var first = Math.Ceiling((end - 1e-8) * moving.FrameRate) / moving.FrameRate;
            var last = Math.Floor((range.Start - moving.KeptDuration + 1e-8) * moving.FrameRate) / moving.FrameRate;
            if (last >= first) candidates.Add(Math.Clamp(requested, first, last));
            end = Math.Max(end, range.End);
        }
        candidates.Add(Math.Max(requested, Math.Ceiling((end - 1e-8) * moving.FrameRate) / moving.FrameRate));
        return candidates.MinBy(c => Math.Abs(c - requested));
    }
    public static List<MediaClip> JoinedFollowing(IReadOnlyList<MediaClip> clips, MediaClip clip)
    {
        var following = new List<MediaClip>();
        var index = clips.ToList().IndexOf(clip);
        var end = Offset(clips, clip) + clip.KeptDuration;
        for (var i = index + 1; i < clips.Count && Math.Abs(Offset(clips, clips[i]) - end) < .00001; i++)
        { following.Add(clips[i]); end += clips[i].KeptDuration; }
        return following;
    }
}
