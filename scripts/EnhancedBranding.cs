using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Text.RegularExpressions;
using System.IO;
public static class EnhancedBrand {
 public const string Star="M50,14 L58,27 L73,19 L72,35 L88,38 L77,50 L87,62 L71,64 L69,78 L56,71 L38,88 L40,71 L26,74 L30,60 L14,52 L27,43 L18,29 L35,31 L37,17 Z";
 public const string Ribbon="M47 39 H67 L63 44 H48 C44 44 42 46 42 48 C42 49 43 50 46 50 H62 L58 55 H44 C40 55 38 57 38 59 C38 60 39 61 42 61 H59 L55 66 H40 C32 66 31 63 33 57 C34 54 36 52 38 51 C34 49 35 46 38 43 C40 40 43 39 47 39 Z";
 static float N(MatchCollection t,ref int i){return float.Parse(t[i++].Value,System.Globalization.CultureInfo.InvariantCulture);}
 static GraphicsPath Path(string data){
  var t=Regex.Matches(data,@"[MLHVCZ]|-?\d+(?:\.\d+)?");int i=0;var p=new GraphicsPath();float x=0,y=0;
  while(i<t.Count){string c=t[i++].Value;float a,b;
   switch(c){
    case "M":x=N(t,ref i);y=N(t,ref i);p.StartFigure();break;
    case "L":a=N(t,ref i);b=N(t,ref i);p.AddLine(x,y,a,b);x=a;y=b;break;
    case "H":a=N(t,ref i);p.AddLine(x,y,a,y);x=a;break;
    case "V":b=N(t,ref i);p.AddLine(x,y,x,b);y=b;break;
    case "C":a=N(t,ref i);b=N(t,ref i);float d=N(t,ref i),e=N(t,ref i),f=N(t,ref i),g=N(t,ref i);p.AddBezier(x,y,a,b,d,e,f,g);x=f;y=g;break;
    case "Z":p.CloseFigure();break;
    default:throw new InvalidDataException("Unknown command "+c);
   }
  }return p;
 }
 public static Bitmap Render(int size){
  int high=size*3;var canvas=new Bitmap(high,high,PixelFormat.Format32bppArgb);
  using(var g=Graphics.FromImage(canvas))using(var star=Path(Star))using(var ribbon=Path(Ribbon)){
   g.Clear(Color.Transparent);g.SmoothingMode=SmoothingMode.AntiAlias;float k=high/78f;
   using(var transform=new Matrix(k,0,0,k,-12*k,-12*k))g.Transform=transform;
   var gold=Color.FromArgb(255,191,71);
   using(var brush=new SolidBrush(gold))using(var pen=new Pen(gold,3)){
    pen.LineJoin=LineJoin.Round;g.FillPath(brush,star);g.DrawPath(pen,star);
   }
   using(var translate=new Matrix()){translate.Translate(2,-1.5f);ribbon.Transform(translate);}
   using(var dark=new SolidBrush(Color.FromArgb(29,22,7)))g.FillPath(dark,ribbon);
  }
  var result=new Bitmap(size,size,PixelFormat.Format32bppArgb);
  using(var g=Graphics.FromImage(result)){
   g.CompositingMode=CompositingMode.SourceCopy;g.InterpolationMode=InterpolationMode.HighQualityBicubic;g.PixelOffsetMode=PixelOffsetMode.HighQuality;
   g.DrawImage(canvas,new Rectangle(0,0,size,size),0,0,high,high,GraphicsUnit.Pixel);
  }canvas.Dispose();return result;
 }
 public static void Icon(string path){
  int[] sizes={16,20,24,32,40,48,64,128,256};var images=new byte[sizes.Length][];
  for(int n=0;n<sizes.Length;n++){
   int side=sizes[n],stride=((side+31)/32)*4;
   using(var bmp=Render(side))using(var mem=new MemoryStream())using(var w=new BinaryWriter(mem)){
    w.Write(40);w.Write(side);w.Write(side*2);w.Write((short)1);w.Write((short)32);w.Write(0);w.Write(side*side*4);w.Write(0);w.Write(0);w.Write(0);w.Write(0);
    for(int y=side-1;y>=0;y--)for(int x=0;x<side;x++){var c=bmp.GetPixel(x,y);w.Write(c.B);w.Write(c.G);w.Write(c.R);w.Write(c.A);}
    for(int y=side-1;y>=0;y--){var mask=new byte[stride];for(int x=0;x<side;x++)if(bmp.GetPixel(x,y).A==0)mask[x/8]|=(byte)(128>>(x%8));w.Write(mask);}
    images[n]=mem.ToArray();
   }
  }
  using(var w=new BinaryWriter(File.Create(path))){
   w.Write((short)0);w.Write((short)1);w.Write((short)sizes.Length);int offset=6+16*sizes.Length;
   for(int n=0;n<sizes.Length;n++){w.Write((byte)(sizes[n]==256?0:sizes[n]));w.Write((byte)(sizes[n]==256?0:sizes[n]));w.Write((byte)0);w.Write((byte)0);w.Write((short)1);w.Write((short)32);w.Write(images[n].Length);w.Write(offset);offset+=images[n].Length;}
   foreach(var bytes in images)w.Write(bytes);
  }
 }
}
