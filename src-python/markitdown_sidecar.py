import sys
from markitdown import MarkItDown

def main():
    path = sys.argv[1]
    md = MarkItDown()
    result = md.convert(path)
    print(result.text_content)

if __name__ == "__main__":
    main()
